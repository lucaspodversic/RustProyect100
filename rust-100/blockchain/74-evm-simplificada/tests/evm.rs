use evm_simplificada::asm::ensamblar;
use evm_simplificada::u256::U256;
use evm_simplificada::vm::*;

fn correr(src: &str, gas: u64) -> (Resultado, Storage) {
    let code = ensamblar(src).expect("asm");
    let mut st = Storage::new();
    let r = ejecutar(&code, &Entorno { limite_gas: gas, remitente: U256::from_u64(0xCAFE), ..Default::default() }, &mut st);
    (r, st)
}

fn palabra(r: &Resultado) -> U256 {
    U256::from_be_bytes(&r.salida)
}

const RET: &str = "PUSH0 MSTORE PUSH1 32 PUSH0 RETURN";

#[test]
fn u256_aritmetica_modular() {
    assert_eq!(U256::MAX.wrapping_add(U256::ONE), U256::ZERO);
    assert_eq!(U256::ZERO.wrapping_sub(U256::ONE), U256::MAX);
    let dos_128 = U256::ONE.shl(128);
    assert_eq!(dos_128.wrapping_mul(dos_128), U256::ZERO, "2^256 mod 2^256 = 0");
    assert_eq!(U256::from_u64(2).wrapping_pow(U256::from_u64(255)), U256::ONE.shl(255));
    let (q, r) = U256::MAX.div_rem(U256::from_u64(10));
    assert_eq!(q.wrapping_mul(U256::from_u64(10)).wrapping_add(r), U256::MAX);
    assert_eq!(r, U256::from_u64(5)); // 2^256-1 termina en ...935
    assert_eq!(U256::from_u64(7).div_rem(U256::ZERO), (U256::ZERO, U256::ZERO));
    assert_eq!(U256::ONE.shl(200).shr(199), U256::from_u64(2));
    let x = U256::from_be_bytes(&hex::decode("0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef").unwrap());
    assert_eq!(hex::encode(x.to_be_bytes()), "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef");
    assert_eq!(format!("{:?}", U256::from_u64(255)), "0xff");
}

#[test]
fn suma_y_resta_con_orden_de_pila_evm() {
    // SUB calcula top - segundo: PUSH 3 PUSH 10 SUB = 10 - 3.
    let (r, _) = correr(&format!("PUSH1 3 PUSH1 10 SUB {RET}"), 1000);
    assert_eq!(r.estado, Estado::Exito);
    assert_eq!(palabra(&r), U256::from_u64(7));
    let (r, _) = correr(&format!("PUSH1 2 PUSH1 3 EXP {RET}"), 1000);
    assert_eq!(palabra(&r), U256::from_u64(9));
}

#[test]
fn factorial_con_bucle_y_saltos() {
    let src = format!(
        "PUSH1 1 PUSH1 20 bucle: DUP1 ISZERO @fin JUMPI DUP1 SWAP2 MUL SWAP1 PUSH1 1 SWAP1 SUB @bucle JUMP fin: POP {RET}"
    );
    let (r, _) = correr(&src, 100_000);
    assert_eq!(palabra(&r).as_u64(), Some(2_432_902_008_176_640_000)); // 20!
}

#[test]
fn keccak_de_vacio() {
    let (r, _) = correr(&format!("PUSH0 PUSH0 KECCAK256 {RET}"), 1000);
    assert_eq!(hex::encode(&r.salida), "c5d2460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470");
}

#[test]
fn contador_persistente_en_storage() {
    // slot0 += 1 ; devuelve el nuevo valor.
    let code = ensamblar(&format!("PUSH0 SLOAD PUSH1 1 ADD DUP1 PUSH0 SSTORE {RET}")).unwrap();
    let mut st = Storage::new();
    let env = Entorno { limite_gas: 50_000, ..Default::default() };
    let r1 = ejecutar(&code, &env, &mut st);
    let r2 = ejecutar(&code, &env, &mut st);
    assert_eq!(palabra(&r2), U256::from_u64(2));
    assert_eq!(st[&U256::ZERO], U256::from_u64(2));
    assert!(r1.gas_usado > r2.gas_usado, "el primer SSTORE (0→≠0) es más caro");
}

#[test]
fn revert_descarta_storage_y_devuelve_datos() {
    let code = ensamblar("PUSH1 42 PUSH0 SSTORE PUSH1 0xee PUSH0 MSTORE8 PUSH1 1 PUSH0 REVERT").unwrap();
    let mut st = Storage::new();
    let r = ejecutar(&code, &Entorno { limite_gas: 50_000, ..Default::default() }, &mut st);
    assert_eq!(r.estado, Estado::Revertido);
    assert_eq!(r.salida, vec![0xee]);
    assert!(st.is_empty());
    assert!(r.gas_usado < 50_000, "REVERT no consume todo el gas");
}

#[test]
fn sin_gas_consume_todo_y_no_persiste() {
    let code = ensamblar("PUSH1 1 PUSH0 SSTORE bucle: @bucle JUMP").unwrap();
    let mut st = Storage::new();
    let r = ejecutar(&code, &Entorno { limite_gas: 30_000, ..Default::default() }, &mut st);
    assert_eq!(r.estado, Estado::Fallo(ErrorEvm::SinGas));
    assert_eq!(r.gas_usado, 30_000);
    assert!(st.is_empty());
}

#[test]
fn salto_dentro_de_datos_de_push_es_invalido() {
    // En 0x02 hay un byte 0x5b, pero es dato de PUSH1, no un JUMPDEST real.
    let code = vec![0x60, 0x02, 0x56, 0x60, 0x5b];
    let code2 = [vec![0x60, 0x04, 0x56], code[3..].to_vec()].concat(); // salta a pc=4 (dato 0x5b)
    let r = ejecutar(&code2, &Entorno { limite_gas: 1000, ..Default::default() }, &mut Storage::new());
    assert_eq!(r.estado, Estado::Fallo(ErrorEvm::SaltoInvalido(4)));
    assert_eq!(analizar_jumpdests(&[0x60, 0x5b, 0x5b]), vec![false, false, true]);
}

#[test]
fn errores_de_pila_y_opcode() {
    let (r, _) = correr("ADD", 100);
    assert!(matches!(r.estado, Estado::Fallo(ErrorEvm::PilaVacia(0))));
    let mut code = vec![];
    for _ in 0..1025 {
        code.push(0x5f); // PUSH0
    }
    let r = ejecutar(&code, &Entorno { limite_gas: 10_000, ..Default::default() }, &mut Storage::new());
    assert!(matches!(r.estado, Estado::Fallo(ErrorEvm::PilaLlena(1024))));
    let r = ejecutar(&[0xfe], &Entorno { limite_gas: 10, ..Default::default() }, &mut Storage::new());
    assert_eq!(r.estado, Estado::Fallo(ErrorEvm::OpcodeInvalido(0xfe)));
}

#[test]
fn calldata_caller_y_logs() {
    // Emite LOG1(topic=caller) con data = calldata[0..32] y devuelve calldata+1.
    let code = ensamblar(&format!(
        "PUSH0 CALLDATALOAD DUP1 PUSH0 MSTORE CALLER PUSH1 32 PUSH0 LOG1 PUSH1 1 ADD {RET}"
    ))
    .unwrap();
    let mut cd = vec![0u8; 32];
    cd[31] = 41;
    let env = Entorno { limite_gas: 10_000, calldata: cd.clone(), remitente: U256::from_u64(0xCAFE), ..Default::default() };
    let r = ejecutar(&code, &env, &mut Storage::new());
    assert_eq!(palabra(&r), U256::from_u64(42));
    assert_eq!(r.logs, vec![Log { topics: vec![U256::from_u64(0xCAFE)], data: cd }]);
}

#[test]
fn memoria_cobra_expansion_cuadratica() {
    let (chico, _) = correr("PUSH1 1 PUSH0 MSTORE", 100_000);
    let (grande, _) = correr("PUSH1 1 PUSH2 0x2000 MSTORE", 100_000);
    assert!(grande.gas_usado > chico.gas_usado * 50);
    let (r, _) = correr("PUSH1 1 PUSH32 0xffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff MSTORE", 100_000);
    assert_eq!(r.estado, Estado::Fallo(ErrorEvm::SinGas));
}

#[test]
fn ensamblador_reporta_errores() {
    assert!(ensamblar("FOO").is_err());
    assert!(ensamblar("PUSH1 0x1234").is_err());
    assert!(ensamblar("@nada JUMP").is_err());
    assert_eq!(ensamblar("PUSH2 300 DUP16 SWAP1 LOG4").unwrap(), vec![0x61, 0x01, 0x2c, 0x8f, 0x90, 0xa4]);
}
