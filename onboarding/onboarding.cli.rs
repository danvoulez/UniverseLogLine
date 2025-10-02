// LogLineID Onboarding CLI - Executa onboarding computável com dados reais

use logline_id::motor::*;
use std::env;

fn main() {
    let handle = env::args().nth(1).expect("Forneça um handle (@danvoulez)");
    let origin = "humano";
    let status = "validado";
    let uuid = generate_uuid();
    let timestamp = current_timestamp();

    let identity = Identity {
        handle: handle.clone(),
        internal_id: format!("logline-id://{}", handle.trim_start_matches('@')),
        origin: origin.to_string(),
        status: status.to_string(),
        created_at: timestamp,
        spans: vec![Span::new("identity_created", uuid.clone(), &handle, &timestamp)],
        permissions: vec!["view timeline".to_string(), "create spans".to_string(), "assinar contratos".to_string()],
    };

    persist_identity(identity);
    println!("Identidade computável criada e registrada para {}", handle);
}

// A função persist_identity persiste no banco computável e registra span na timeline.