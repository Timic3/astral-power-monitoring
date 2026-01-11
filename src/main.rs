mod monitor;

use monitor::AstralPowerMonitor;

fn main() {
    println!("NVIDIA RTX Astral Pin Power Monitor");
    println!("=========================================\n");

    // Initialize NVAPI
    let monitor = match AstralPowerMonitor::new() {
        Ok(m) => m,
        Err(e) => {
            eprintln!("Error initializing NVAPI: {}", e);
            eprintln!("\nMake sure you have:");
            eprintln!("  1. An NVIDIA GPU installed");
            eprintln!("  2. NVIDIA drivers installed");
            eprintln!("  3. Running on Windows");
            std::process::exit(1);
        }
    };

    println!("Found {} NVIDIA GPU(s)\n", monitor.gpu_count());

    // Continuous monitoring loop
    let gpu_index = 0;
    let mut voltages = [0.0f32; 6];
    let mut currents = [0.0f32; 6];
    let mut first_iteration = true;

    loop {
        match monitor.get_power_status(gpu_index, &mut voltages, &mut currents) {
            Ok(()) => {
                use std::io::Write;

                if !first_iteration {
                    // Move cursor up 8 lines to overwrite previous data
                    print!("\x1b[8A");
                } else {
                    println!("GPU {} Power Rail Status:", gpu_index);
                    println!("==========================");
                    first_iteration = false;
                }

                let mut total_power = 0.0f32;
                for i in 0..6 {
                    let power = voltages[i] * currents[i];
                    total_power += power;

                    // When do they start melting?
                    let current_color = if currents[i] >= 9.0 {
                        "\x1b[91m" // Bright red
                    } else if currents[i] >= 6.0 {
                        "\x1b[93m" // Bright yellow
                    } else {
                        "\x1b[92m" // Bright green
                    };

                    print!("  Pin {}: {:.3}V × {}{:.2}A{} = {:.2}W",
                           i + 1, voltages[i], current_color, currents[i], "\x1b[0m", power);
                    println!("\x1b[K"); // Clear to end of line
                }

                println!("\x1b[K"); // Clear blank line
                print!("  Total Power: {:.2}W", total_power);
                println!("\x1b[K"); // Clear to end of line

                std::io::stdout().flush().unwrap();
            }
            Err(e) => {
                eprintln!("\nError: {}", e);
                break;
            }
        }

        std::thread::sleep(std::time::Duration::from_secs(1));
    }
}
