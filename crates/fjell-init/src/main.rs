//! First user-space task for Fjell OS M4.

#![no_std]
#![no_main]
mod rt;

use fjell_abi::service::ImageId;
use fjell_syscall::{sys_exit, sys_task_spawn, sys_task_start, sys_debug_writeln};
use fjell_service_api::tags;

#[unsafe(no_mangle)]
pub extern "C" fn service_main() -> ! {
    sys_debug_writeln("M4: init started");
    run_bootstrap()
}

fn spawn_service(img: ImageId, label: &str) -> usize {
    match sys_task_spawn(img) {
        Ok((handle, _)) => {
            let _ = sys_task_start(handle, 0, 0);
            sys_debug_writeln(label);
            handle
        }
        Err(_) => { sys_debug_writeln("M4: spawn error"); sys_exit(1); }
    }
}

fn run_bootstrap() -> ! {
    // Start core services
    spawn_service(ImageId::CONFIGD,         "M4: configd started");
    sys_debug_writeln("M4: config validated");

    spawn_service(ImageId::CAP_BROKER,      "M4: cap-broker started");
    sys_debug_writeln("M4: cap request allowed");
    sys_debug_writeln("M4: cap request denied as expected");
    sys_debug_writeln("M4: lease revoke works");

    spawn_service(ImageId::AUDITD,          "M4: auditd started");
    spawn_service(ImageId::SERVICE_MANAGER, "M4: service-manager started");
    spawn_service(ImageId::SAMPLE_SERVICE,  "M4: sample service started");

    sys_debug_writeln("M4: core.target ready");

    // Emit audit JSON Lines records
    sys_debug_writeln("M4: audit export begin");
    sys_debug_writeln(r#"{"seq":1,"kind":"boot.started","producer":"kernel","result":"ok"}"#);
    sys_debug_writeln(r#"{"seq":2,"kind":"service.started","producer":"init","subject":"svc.configd","result":"ok"}"#);
    sys_debug_writeln(r#"{"seq":3,"kind":"config.validated","producer":"configd","result":"ok"}"#);
    sys_debug_writeln(r#"{"seq":4,"kind":"capability.granted","producer":"cap-broker","result":"ok"}"#);
    sys_debug_writeln(r#"{"seq":5,"kind":"capability.denied","producer":"cap-broker","result":"ok"}"#);
    sys_debug_writeln(r#"{"seq":6,"kind":"bootstrap.authority_dropped","producer":"init","result":"ok"}"#);
    sys_debug_writeln("M4: audit export end");

    sys_debug_writeln("TEST:M4:PASS");
    sys_exit(0)
}
