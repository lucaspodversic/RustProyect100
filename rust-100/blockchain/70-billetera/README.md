# 70 · Billetera de criptomonedas

Billetera secp256k1 con **criptografía real** (RustCrypto: `k256`, `sha3`, `sha2`, `ripemd`):

- Genera claves con la entropía del SO, importa/exporta en hex (el `Debug` nunca muestra la privada).
- Direcciones **Ethereum** con checksum **EIP-55** y **Bitcoin P2PKH** (Base58Check).
- Firma de transacciones legacy con protección de replay **EIP-155** (RLP propio + Keccak-256 + ECDSA determinista RFC 6979, `s` bajo EIP-2).
- Parseo de tx raw y **ecrecover** del remitente; firma de mensajes **EIP-191**.

## Tests
Verificados contra **vectores oficiales**: ejemplo exacto de EIP-155 (raw tx byte a byte), direcciones de EIP-55, dirección BTC de la clave privada `1`, y casos RLP de la wiki de Ethereum.

```bash
cargo test
cargo run -- nueva
cargo run -- firmar 0x4646…46 0x3535353535353535353535353535353535353535 1000000000000000000 9
cargo run -- verificar 0xf86c…
```

## Limitaciones
Sin BIP-39/BIP-32 (mnemónicos/HD), sin keystore cifrado ni transacciones EIP-1559. Uso educativo: no guardes fondos reales con esto.
