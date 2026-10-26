//! CLI mínima:
//!   billetera nueva
//!   billetera direccion <clave_hex>
//!   billetera firmar <clave_hex> <to_hex> <wei> <nonce> [chain_id]
//!   billetera verificar <raw_tx_hex>

use billetera::claves::{validar_direccion_eth, ParClaves};
use billetera::tx::{Transaccion, TxFirmada};

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let r = match a.first().map(String::as_str) {
        Some("nueva") | None => {
            let k = ParClaves::generar();
            println!("Clave privada: 0x{}  (¡guardala en secreto!)", k.exportar_hex());
            println!("Dirección ETH: {}", k.direccion_eth());
            println!("Dirección BTC: {}", k.direccion_btc());
            Ok(())
        }
        Some("direccion") if a.len() >= 2 => ParClaves::desde_hex(&a[1]).map(|k| println!("{}\n{}", k.direccion_eth(), k.direccion_btc())).map_err(|e| e.to_string()),
        Some("firmar") if a.len() >= 5 => (|| {
            let k = ParClaves::desde_hex(&a[1]).map_err(|e| e.to_string())?;
            let to = validar_direccion_eth(&a[2]).map_err(|e| e.to_string())?;
            let tx = Transaccion { nonce: a[4].parse().map_err(|_| "nonce")?, gas_price: 20_000_000_000, gas_limit: 21_000, to: Some(to), value: a[3].parse().map_err(|_| "wei")?, data: vec![], chain_id: a.get(5).and_then(|s| s.parse().ok()).unwrap_or(1) };
            let f = tx.firmar(&k);
            println!("raw:  0x{}\nhash: 0x{}", hex::encode(f.rlp()), hex::encode(f.hash()));
            Ok::<(), String>(())
        })(),
        Some("verificar") if a.len() >= 2 => (|| {
            let raw = hex::decode(a[1].trim_start_matches("0x")).map_err(|_| "hex")?;
            let f = TxFirmada::desde_rlp(&raw).map_err(|e| e.to_string())?;
            let from = f.recuperar_remitente().map_err(|e| e.to_string())?;
            println!("remitente: {}  chain_id: {}  valor: {}", billetera::claves::checksum_eip55(&from), f.tx.chain_id, f.tx.value);
            Ok::<(), String>(())
        })(),
        _ => Err("uso: nueva | direccion <clave> | firmar <clave> <to> <wei> <nonce> [chain] | verificar <raw>".into()),
    };
    if let Err(e) = r {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}
