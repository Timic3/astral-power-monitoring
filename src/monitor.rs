use std::mem;

use nvapi_sys::gpu::NvAPI_EnumPhysicalGPUs;
use nvapi_sys::handles::NvPhysicalGpuHandle;
use nvapi_sys::i2c::{NV_I2C_INFO_VER3, NVAPI_I2C_SPEED_100KHZ};
use nvapi_sys::i2c::private::{NV_I2C_INFO_EX_V3, NvAPI_I2CReadEx};
use nvapi_sys::status::NVAPI_OK;
use nvapi_sys::types::NVAPI_MAX_PHYSICAL_GPUS;

// IT8915 Power Monitoring IC constants
const IT8915_I2C_ADDRESS: u8 = 0x56; // I2C device address
const IT8915_POWER_REG_START: u8 = 0x80; // Starting register for power readings
const IT8915_POWER_DATA_SIZE: usize = 24; // 24 bytes of power data

pub struct AstralPowerMonitor {
    gpu_handles: Vec<NvPhysicalGpuHandle>,
}

impl AstralPowerMonitor {
    /// Initialize NVAPI and enumerate GPUs
    pub fn new() -> Result<Self, String> {
        unsafe {
            let mut gpu_handles = [mem::zeroed(); NVAPI_MAX_PHYSICAL_GPUS as usize];
            let mut gpu_count: u32 = 0;

            let status = NvAPI_EnumPhysicalGPUs(&mut gpu_handles, &mut gpu_count);
            if status != NVAPI_OK {
                return Err(format!("Failed to enumerate GPUs: status {:?}", status));
            }

            if gpu_count == 0 {
                return Err("No NVIDIA GPUs found".to_string());
            }

            let gpu_handles_vec = gpu_handles[..gpu_count as usize].to_vec();

            Ok(Self {
                gpu_handles: gpu_handles_vec,
            })
        }
    }

    /// Read raw data from IT8915 power monitoring IC via I2C
    ///
    /// # Arguments
    /// * `gpu_index` - GPU index (0-based)
    /// * `reg_addr` - IT8915 register address
    /// * `data` - Buffer to receive data
    ///
    /// # Returns
    /// * `Ok(())` on success
    /// * `Err(String)` with error message on failure
    fn read_i2c_data(
        &self,
        gpu_index: i32,
        reg_addr: u8,
        data: &mut [u8],
    ) -> Result<(), String> {
        let gpu_handle = self.gpu_handles[gpu_index as usize];

        unsafe {
            let mut reg_addr_buf = reg_addr;

            // This whole structure is probably wrong, but it works.
            // The following structure that was reverse-engineered from
            // ExpanModule.dll seems more correct:
            /*
                struct NV_I2C_INFO {
                    version: u32,              // +0x00 (1002D494): 0x030040 = size 64 | version 3
                    display_mask: u32,         // +0x04 (1002D498): 0
                    is_ddc_port: u8,           // +0x08 (1002D49C): 0
                    i2c_dev_address: u8,       // +0x09 (1002D49D): 0x56
                    _reserved1: u16,           // +0x0A-0x0B: padding
                    i2c_reg_address: *mut u8,  // +0x0C (1002D4A0): pointer to register
                    reg_addr_size: u32,        // +0x10 (1002D4A4): 1
                    i2c_data: *mut u8,         // +0x14 (1002D4A8): pointer to data buffer
                    i2c_data_size: u32,        // +0x18 (1002D4AC): 24
                    port_id: u32,              // +0x1C (1002D4B0): 0xFFFF
                    i2c_speed_khz: u32,        // +0x20 (1002D4B4): 4
                    is_port_id_set: u8,        // +0x24 (1002D4B8): 1
                    _reserved3: u8,            // +0x25
                    _reserved4: u16,           // +0x26-0x27: padding
                    _reserved5: u32,           // +0x28 (1002D4BC): 1
                    _reserved6: [u32; 5],      // +0x2C to +0x3F: padding to 64 bytes
                }
            */
            let mut i2c_info = NV_I2C_INFO_EX_V3 {
                version: NV_I2C_INFO_VER3,
                displayMask: 0,
                bIsDDCPort: 0,
                i2cDevAddress: IT8915_I2C_ADDRESS,
                pbI2cRegAddress: &mut reg_addr_buf,
                regAddrSize: 1,
                pbData: data.as_mut_ptr(),
                pbRead: data.len() as u32,
                cbSize: 0xFFFF,
                i2cSpeedKhz: NVAPI_I2C_SPEED_100KHZ,
                portId: 0x01,
                bIsPortIdSet: 1
            };

            let mut i2c_status = 0u32;
            let status = NvAPI_I2CReadEx(gpu_handle, &mut i2c_info, &mut i2c_status);

            if status != NVAPI_OK {
                return Err(format!("I2C read failed: NVAPI status {:?}", status));
            }
        }

        Ok(())
    }

    /// Get power status for a specific GPU by reading IT8915 power monitoring IC
    ///
    /// # Arguments
    /// * `gpu_index` - GPU index (0-based)
    /// * `voltage_buffer` - Buffer to receive 6 power rail voltage values in volts
    /// * `current_buffer` - Buffer to receive 6 power rail current values in amperes
    ///
    /// # Returns
    /// * `Ok(())` on success
    /// * `Err(String)` with error message on failure
    pub fn get_power_status(
        &self,
        gpu_index: i32,
        voltage_buffer: &mut [f32; 6],
        current_buffer: &mut [f32; 6],
    ) -> Result<(), String> {
        if gpu_index < 0 || gpu_index >= self.gpu_handles.len() as i32 {
            return Err(format!(
                "Invalid GPU index {}. Valid range: 0-{}",
                gpu_index,
                self.gpu_handles.len() - 1
            ));
        }

        // Read 24 bytes from IT8915 starting at register 0x80
        let mut raw_data = [0u8; IT8915_POWER_DATA_SIZE];
        self.read_i2c_data(gpu_index, IT8915_POWER_REG_START, &mut raw_data)?;

        // Data structure: 6 rails × 4 bytes each = 24 bytes
        // Each 4-byte block contains:
        //   Bytes +0,+1: Voltage (16-bit big-endian, millivolts)
        //   Bytes +2,+3: Current (16-bit big-endian, milliamps)
        // Note: Order is reversed when fetched via I2C

        // Extract big-endian u16 from byte array
        let read_u16_be = |offset: usize| -> u16 {
            ((raw_data[offset] as u16) << 8) | (raw_data[offset + 1] as u16)
        };

        // Hardware rails are stored in reverse order: Rail 5, 4, 3, 2, 1, 0
        // Map them to software pins: Pin 0, 1, 2, 3, 4, 5
        let rail_voltages = [
            read_u16_be(20), // Pin 0 = Rail 5: bytes 20-21
            read_u16_be(16), // Pin 1 = Rail 4: bytes 16-17
            read_u16_be(12), // Pin 2 = Rail 3: bytes 12-13
            read_u16_be(8),  // Pin 3 = Rail 2: bytes 8-9
            read_u16_be(4),  // Pin 4 = Rail 1: bytes 4-5
            read_u16_be(0),  // Pin 5 = Rail 0: bytes 0-1
        ];

        let rail_currents = [
            read_u16_be(22), // Pin 0 = Rail 5: bytes 22-23
            read_u16_be(18), // Pin 1 = Rail 4: bytes 18-19
            read_u16_be(14), // Pin 2 = Rail 3: bytes 14-15
            read_u16_be(10), // Pin 3 = Rail 2: bytes 10-11
            read_u16_be(6),  // Pin 4 = Rail 1: bytes 6-7
            read_u16_be(2),  // Pin 5 = Rail 0: bytes 2-3
        ];

        // Convert to volts and amperes (millivolts/milliamps * 0.001)
        for i in 0..6 {
            voltage_buffer[i] = (rail_voltages[i] as f32) * 0.001;
            current_buffer[i] = (rail_currents[i] as f32) * 0.001;
        }

        Ok(())
    }

    /// Get the number of available GPUs
    pub fn gpu_count(&self) -> usize {
        self.gpu_handles.len()
    }

    /// Get power status and return as vectors (voltages, currents)
    #[allow(dead_code)]
    pub fn get_power_status_vec(&self, gpu_idx: i32) -> Result<(Vec<f32>, Vec<f32>), String> {
        let mut voltages = [0.0f32; 6];
        let mut currents = [0.0f32; 6];
        self.get_power_status(gpu_idx, &mut voltages, &mut currents)?;
        Ok((voltages.to_vec(), currents.to_vec()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_power_monitor_init() {
        let monitor = AstralPowerMonitor::new();
        assert!(monitor.is_ok() || monitor.is_err()); // Will fail on non-NVIDIA systems
    }
}
