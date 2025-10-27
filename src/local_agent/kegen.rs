use openssl::rsa::Rsa;
use std::{fs, path::PathBuf};

fn generate_rsa_keypair() -> anyhow::Result<(Vec<u8>, Vec<u8>)> {
    // Generate a 2048-bit RSA key
    let rsa = Rsa::generate(2048)?;

    // Private key in PKCS#8 PEM (preferred)
    let private_pem = rsa.private_key_to_pem()?; // PKCS#8 PEM
    // Public key in PKCS#1 PEM
    let public_pem = rsa.public_key_to_pem_pkcs1()?; // PKCS#1 PEM

    Ok((private_pem, public_pem))
}

pub async fn create_key_pair() -> anyhow::Result<()> {
    let private_key = PathBuf::from("private_key.pk8.pem");
    if !private_key.exists() {
        // TODO: Check expiry
        let (priv_pem, pub_pem) = generate_rsa_keypair()?;
        fs::write("private_key.pk8.pem", &priv_pem)?;
        fs::write("public_key.pk1.pem", &pub_pem)?;

        // TODO: Send pub to relay with some identifier
    };
    Ok(())
}
