use std::process::{Command, Stdio};
use std::time::Duration;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::time::SystemTime;

fn main() {
    let server_path = "/home/tdy/Escritorio/dmart/target/release/dmart-server";
    let log_file = "/home/tdy/Escritorio/dmart/launcher.log";
    
    if !Path::new(server_path).exists() {
        eprintln!("Error: No se encontro el binario dmart-server");
        eprintln!("Ejecuta primero: cd /home/tdy/Escritorio/dmart && cargo build --release");
        return;
    }
    
    println!("===========================================");
    println!("  dMart UCI - Sistema de Gestion");
    println!("===========================================");
    println!("");
    
    // Escribir al log
    let mut log = match OpenOptions::new().create(true).append(true).open(log_file) {
        Ok(f) => f,
        Err(_) => File::create(log_file).unwrap(),
    };
    let _ = writeln!(log, "[SYSTEM] Iniciando dMart launcher...");
    
    loop {
        println!("[START] Iniciando dmart-server...");
        println!("[INFO] Servidor en http://localhost:3000");
        let _ = writeln!(log, "[{}] Iniciando servidor", timestamp());
        let _ = log.flush();
        
        // Iniciar con setsid (nueva sesion, no recibe senales del padre)
        let _child = match Command::new("setsid")
            .arg(server_path)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
        {
            Ok(c) => c,
            Err(e) => {
                eprintln!("[ERROR] No se pudo iniciar: {}", e);
                let _ = writeln!(log, "[ERROR] {}", e);
                std::thread::sleep(Duration::from_secs(5));
                continue;
            }
        };
        
        println!("[OK] Servidor corriendo");
        let _ = writeln!(log, "[OK] Servidor iniciado");
        let _ = log.flush();
        
        // Esperar 60 segundos o hasta que se caiga
        std::thread::sleep(Duration::from_secs(60));
        
        println!("[CHECK] Verificando servidor...");
        
        // Verificar si esta respondiendo
        match Command::new("curl")
            .args(["-s", "-o", "/dev/null", "-w", "%{http_code}", "http://localhost:3000/"])
            .output()
        {
            Ok(output) => {
                if String::from_utf8_lossy(&output.stdout).contains("200") {
                    println!("[OK] Servidor respondiendo");
                    // Seguir corriendo, no reiniciar
                    std::thread::sleep(Duration::from_secs(60));
                    continue;
                }
            }
            Err(_) => {}
        }
        
        println!("[WARN] Servidor no responde, reiniciando...");
        let _ = writeln!(log, "[WARN] No responde, reiniciando...");
        std::thread::sleep(Duration::from_secs(2));
    }
}

fn timestamp() -> String {
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap();
    format!("{}", now.as_secs())
}