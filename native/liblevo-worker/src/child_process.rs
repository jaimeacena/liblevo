//! Procesos hijo de los lectores (Xberg, modelo visual): quedan atados a la app para que Windows
//! los detenga si la app muere, también si se cierra a la fuerza.

/// Grupo de procesos de Windows propio de la app, creado una vez. Windows lo cierra cuando la app
/// termina, también si se cierra a la fuerza, y entonces detiene los lectores que sigan vivos.
#[cfg(windows)]
pub(crate) fn kill_on_exit_job() -> Option<windows_sys::Win32::Foundation::HANDLE> {
    use std::sync::OnceLock;
    use windows_sys::Win32::System::JobObjects::{
        CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
        JobObjectExtendedLimitInformation, SetInformationJobObject,
    };
    // El identificador se guarda como número porque un puntero crudo no se puede compartir entre
    // hilos; nunca se cierra a propósito.
    static JOB: OnceLock<Option<usize>> = OnceLock::new();
    let job = JOB.get_or_init(|| {
        // SAFETY: llamadas documentadas de Win32 con argumentos válidos; la estructura vive
        // durante la llamada y su tamaño se indica exactamente.
        unsafe {
            let job = CreateJobObjectW(std::ptr::null(), std::ptr::null());
            if job.is_null() {
                return None;
            }
            let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
            limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            let set = SetInformationJobObject(
                job,
                JobObjectExtendedLimitInformation,
                (&raw const limits).cast(),
                size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            );
            (set != 0).then_some(job as usize)
        }
    });
    job.map(|job| job as windows_sys::Win32::Foundation::HANDLE)
}

/// Ata un lector a la app para que no quede huérfano si la app muere durante una conversión.
/// Si Windows no lo permite, la conversión sigue: la cancelación y el tiempo máximo siguen
/// deteniendo el lector mientras la app viva.
#[cfg(windows)]
pub(crate) fn tie_to_this_process(child: &std::process::Child) -> bool {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::System::JobObjects::AssignProcessToJobObject;
    let Some(job) = kill_on_exit_job() else {
        return false;
    };
    // SAFETY: `job` es un grupo válido abierto por este proceso y el proceso hijo sigue abierto
    // mientras exista `child`.
    unsafe { AssignProcessToJobObject(job, child.as_raw_handle()) != 0 }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(windows)]
    #[test]
    fn reader_process_is_tied_to_the_app() {
        use std::os::windows::io::AsRawHandle;
        use windows_sys::Win32::System::JobObjects::{
            IsProcessInJob, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
            JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
            QueryInformationJobObject,
        };
        let comspec = std::env::var("COMSPEC").unwrap_or_else(|_| "cmd.exe".into());
        let mut child = std::process::Command::new(comspec)
            .args(["/c", "ping", "-n", "20", "127.0.0.1"])
            .stdout(std::process::Stdio::null())
            .spawn()
            .expect("proceso de prueba");
        let tied = tie_to_this_process(&child);
        let job = kill_on_exit_job().expect("grupo de procesos");
        let mut inside = 0;
        let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        // SAFETY: identificadores válidos y estructuras del tamaño indicado.
        let (checked, queried) = unsafe {
            (
                IsProcessInJob(child.as_raw_handle(), job, &mut inside),
                QueryInformationJobObject(
                    job,
                    JobObjectExtendedLimitInformation,
                    (&raw mut limits).cast(),
                    size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
                    std::ptr::null_mut(),
                ),
            )
        };
        let _ = child.kill();
        let _ = child.wait();
        assert!(tied && checked != 0 && inside != 0);
        assert!(queried != 0);
        assert_ne!(
            limits.BasicLimitInformation.LimitFlags & JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
            0
        );
    }
}
