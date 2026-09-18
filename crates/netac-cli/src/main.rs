use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "netac-recover")]
#[command(about = "Innogrit IG5236 recovery tool", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    #[arg(short, long, global = true, action = clap::ArgAction::Count, help = "Verbose (-v, -vv, -vvv)")]
    verbose: u8,
}

#[derive(Subcommand)]
enum Commands {
    /// Scan for JMS583 devices
    Scan,
    /// Send SCSI INQUIRY to a JMS583 device
    Inquiry,
    /// Send SCSI TEST UNIT READY
    Tur,
    /// Send SCSI REQUEST SENSE
    Sense,
    /// Read capacity from block device
    Capacity,
    /// NVMe Identify Controller via bridge
    NvmeIdentify,
    /// Print Linux setup instructions
    LinuxSetup,
    /// Enter recovery mode
    Recovery,
    /// Flash firmware from directory
    Flash {
        /// Path to firmware directory (e.g. m2_0536)
        fw_dir: String,
    },
    /// Identify connected controller
    Identify,
    /// Inspect a GRIT firmware image
    Inspect {
        /// Path to .fw file
        path: String,
    },
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Scan => cmd_scan(cli.verbose),
        Commands::Inquiry => cmd_inquiry(cli.verbose),
        Commands::Tur => cmd_tur(cli.verbose),
        Commands::Sense => cmd_sense(cli.verbose),
        Commands::Capacity => cmd_capacity(cli.verbose),
        Commands::NvmeIdentify => cmd_nvme_identify(cli.verbose),
        Commands::LinuxSetup => cmd_linux_setup(),
        Commands::Recovery => cmd_recovery(),
        Commands::Flash { fw_dir } => cmd_flash(&fw_dir),
        Commands::Identify => cmd_identify(),
        Commands::Inspect { path } => cmd_inspect(&path),
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

#[cfg(feature = "uasp")]
fn open_first_jms583(verbose: u8) -> Option<transport::uasp_jms583::UaspDevice> {
    let devices = match transport::uasp_jms583::scan_jms583() {
        Ok(d) if d.is_empty() => {
            eprintln!("No JMS583 devices found.");
            return None;
        }
        Ok(d) => d,
        Err(e) => {
            eprintln!("Scan failed: {e}");
            return None;
        }
    };

    let dev = &devices[0];
    if verbose >= 1 {
        eprintln!("[trace] Opening {} ({:04X}:{:04X})", dev.product, dev.vid, dev.pid);
    }

    match transport::uasp_jms583::UaspDevice::open(dev) {
        Ok(mut d) => {
            d.set_trace_level(verbose);
            Some(d)
        }
        Err(e) => {
            eprintln!("Failed to open device: {e}");
            eprintln!("Try running with sudo.");
            eprintln!("On Linux: sudo modprobe -r uas  OR  echo -n '152d 0583' > /sys/bus/usb/drivers/uas/unbind");
            None
        }
    }
}

// ---------------------------------------------------------------------------
// SCSI commands
// ---------------------------------------------------------------------------

fn cmd_scan(verbose: u8) {
    #[cfg(not(feature = "uasp"))]
    {
        let _ = verbose;
        println!("netac-recover: scan");
        println!("  Build with --features uasp for USB JMS583 support.");
    }

    #[cfg(feature = "uasp")]
    {
        if verbose >= 1 {
            eprintln!("[trace] Enumerating USB devices via rusb (libusb)...");
        }
        match transport::uasp_jms583::scan_jms583() {
            Ok(devices) if devices.is_empty() => {
                println!("No JMS583 devices found.");
            }
            Ok(devices) => {
                for (i, dev) in devices.iter().enumerate() {
                    if i > 0 {
                        println!();
                    }
                    println!("Device #{}", i + 1);
                    println!("{dev}");
                }
            }
            Err(e) => {
                eprintln!("Scan failed: {e}");
                std::process::exit(1);
            }
        }
    }
}

fn cmd_inquiry(verbose: u8) {
    #[cfg(not(feature = "uasp"))]
    {
        let _ = verbose;
        println!("netac-recover: inquiry");
        println!("  Build with --features uasp for USB support.");
    }

    #[cfg(feature = "uasp")]
    {
        let mut uasp = match open_first_jms583(verbose) {
            Some(d) => d,
            None => std::process::exit(1),
        };

        if verbose >= 1 {
            eprintln!("[trace] Sending INQUIRY...");
        }
        match transport::scsi_commands::inquiry(&mut uasp) {
            Ok(info) => {
                println!("{info}");
            }
            Err(e) => {
                eprintln!("INQUIRY failed: {e}");
                if verbose >= 1 {
                    eprintln!("[trace] Check: JMS583 connected? UAS interface claimed?");
                }
            }
        }
    }
}

fn cmd_tur(verbose: u8) {
    #[cfg(not(feature = "uasp"))]
    {
        let _ = verbose;
        println!("netac-recover: tur");
        println!("  Build with --features uasp for USB support.");
    }

    #[cfg(feature = "uasp")]
    {
        let mut uasp = match open_first_jms583(verbose) {
            Some(d) => d,
            None => std::process::exit(1),
        };

        if verbose >= 1 {
            eprintln!("[trace] Sending TEST UNIT READY...");
        }
        match transport::scsi_commands::test_unit_ready(&mut uasp) {
            Ok(()) => {
                println!("Device ready");
            }
            Err(e) => {
                eprintln!("TUR failed: {e}");
            }
        }
    }
}

fn cmd_sense(verbose: u8) {
    #[cfg(not(feature = "uasp"))]
    {
        let _ = verbose;
        println!("netac-recover: sense");
        println!("  Build with --features uasp for USB support.");
    }

    #[cfg(feature = "uasp")]
    {
        let mut uasp = match open_first_jms583(verbose) {
            Some(d) => d,
            None => std::process::exit(1),
        };

        if verbose >= 1 {
            eprintln!("[trace] Sending REQUEST SENSE...");
        }
        match transport::scsi_commands::request_sense(&mut uasp) {
            Ok(sense) => {
                println!("{sense}");
            }
            Err(e) => {
                eprintln!("REQUEST SENSE failed: {e}");
            }
        }
    }
}

fn cmd_capacity(verbose: u8) {
    #[cfg(not(feature = "uasp"))]
    {
        let _ = verbose;
        println!("netac-recover: capacity");
        println!("  Build with --features uasp for USB support.");
    }

    #[cfg(feature = "uasp")]
    {
        let mut uasp = match open_first_jms583(verbose) {
            Some(d) => d,
            None => std::process::exit(1),
        };

        if verbose >= 1 {
            eprintln!("[trace] Sending READ CAPACITY (10)...");
        }
        match transport::scsi_commands::read_capacity(&mut uasp) {
            Ok(cap) => {
                println!("{cap}");
            }
            Err(e) => {
                eprintln!("READ CAPACITY failed: {e}");
            }
        }
    }
}

fn cmd_nvme_identify(verbose: u8) {
    #[cfg(not(feature = "uasp"))]
    {
        let _ = verbose;
        println!("netac-recover: nvme-identify");
        println!("  Build with --features uasp for USB support.");
    }

    #[cfg(feature = "uasp")]
    {
        use transport::NvmeTunnel;

        let uasp = match open_first_jms583(verbose) {
            Some(d) => d,
            None => std::process::exit(1),
        };

        if verbose >= 1 {
            eprintln!("[trace] Sending NVMe Identify Controller via JMS583 0xA1 tunnel...");
        }

        let mut tunnel = transport::nvme_tunnel::Jms583NvmeTunnel::new(uasp);
        let cmd = nvme_core::command::identify_controller();

        let mut data = vec![0u8; 4096];
        match tunnel.nvme_admin(&cmd, &mut data, transport::nvme_tunnel::TunnelDirection::FromDevice) {
            Ok(()) => {
                let id: nvme_core::identify::IdentifyController = unsafe {
                    std::ptr::read(data.as_ptr() as *const _)
                };
                println!("NVMe Identify Controller:");
                println!("  Model:    {}", id.model_number());
                println!("  Serial:   {}", id.serial_number());
                println!("  Firmware: {}", id.firmware_rev());
            }
            Err(e) => {
                eprintln!("NVMe Identify failed: {e:?}");
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Stubs
// ---------------------------------------------------------------------------

fn cmd_recovery() {
    println!("netac-recover: recovery");
    println!("  Not implemented yet.");
}

fn cmd_flash(_fw_dir: &str) {
    println!("netac-recover: flash");
    println!("  Not implemented yet.");
}

fn cmd_identify() {
    println!("netac-recover: identify");
    println!("  Not implemented yet.");
}

fn cmd_inspect(path: &str) {
    let data = match std::fs::read(path) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("Failed to read {path}: {e}");
            std::process::exit(1);
        }
    };

    match innogrit_protocol::grit::GritImage::parse(&data) {
        Ok(image) => {
            println!("{image}");

            if let Ok(sec) = image.get_section_data(&image.sections()[0].name) {
                println!(
                    "\n  First section entropy: {:.2} bits/byte",
                    entropy(sec)
                );
            }
        }
        Err(e) => {
            eprintln!("Parse error: {e}");
        }
    }
}

fn entropy(data: &[u8]) -> f64 {
    if data.is_empty() {
        return 0.0;
    }
    let mut counts = [0u64; 256];
    for &b in data {
        counts[b as usize] += 1;
    }
    let len = data.len() as f64;
    let mut ent = 0.0f64;
    for &c in &counts {
        if c > 0 {
            let p = c as f64 / len;
            ent -= p * p.log2();
        }
    }
    ent
}

fn cmd_linux_setup() {
    println!(r#"
=== Linux Setup for netac-recover ===

1. Verify JMS583 is connected:
   lsusb | grep 152d:0583

2. Unbind kernel UAS driver (pick one):
   Option A — remove module entirely:
     sudo modprobe -r uas

   Option B — unbind only this device:
     echo -n '152d 0583' | sudo tee /sys/bus/usb/drivers/uas/unbind

3. Run diagnostics (with tracing):
   sudo ./netac-recover scan
   sudo ./netac-recover inquiry -v
   sudo ./netac-recover tur -v
   sudo ./netac-recover sense -v
   sudo ./netac-recover capacity -v
   sudo ./netac-recover nvme-identify -v

4. For verbose hex dumps:
   sudo ./netac-recover inquiry -vv       (show CDB + raw response)
   sudo ./netac-recover inquiry -vvv      (show full UASP IU hex)

5. Restore kernel driver when done:
   sudo modprobe uas
   # OR replug the USB device
"#);
}
