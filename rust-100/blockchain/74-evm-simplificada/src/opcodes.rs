//! Tabla de opcodes soportados, costos de gas base y mnemónicos.

pub const STOP: u8 = 0x00;
pub const ADD: u8 = 0x01;
pub const MUL: u8 = 0x02;
pub const SUB: u8 = 0x03;
pub const DIV: u8 = 0x04;
pub const MOD: u8 = 0x06;
pub const EXP: u8 = 0x0a;
pub const LT: u8 = 0x10;
pub const GT: u8 = 0x11;
pub const EQ: u8 = 0x14;
pub const ISZERO: u8 = 0x15;
pub const AND: u8 = 0x16;
pub const OR: u8 = 0x17;
pub const XOR: u8 = 0x18;
pub const NOT: u8 = 0x19;
pub const BYTE: u8 = 0x1a;
pub const SHL: u8 = 0x1b;
pub const SHR: u8 = 0x1c;
pub const KECCAK256: u8 = 0x20;
pub const CALLER: u8 = 0x33;
pub const CALLVALUE: u8 = 0x34;
pub const CALLDATALOAD: u8 = 0x35;
pub const CALLDATASIZE: u8 = 0x36;
pub const POP: u8 = 0x50;
pub const MLOAD: u8 = 0x51;
pub const MSTORE: u8 = 0x52;
pub const MSTORE8: u8 = 0x53;
pub const SLOAD: u8 = 0x54;
pub const SSTORE: u8 = 0x55;
pub const JUMP: u8 = 0x56;
pub const JUMPI: u8 = 0x57;
pub const PC: u8 = 0x58;
pub const MSIZE: u8 = 0x59;
pub const GAS: u8 = 0x5a;
pub const JUMPDEST: u8 = 0x5b;
pub const PUSH0: u8 = 0x5f;
pub const PUSH1: u8 = 0x60;
pub const PUSH32: u8 = 0x7f;
pub const DUP1: u8 = 0x80;
pub const DUP16: u8 = 0x8f;
pub const SWAP1: u8 = 0x90;
pub const SWAP16: u8 = 0x9f;
pub const LOG0: u8 = 0xa0;
pub const LOG4: u8 = 0xa4;
pub const RETURN: u8 = 0xf3;
pub const REVERT: u8 = 0xfd;
pub const INVALID: u8 = 0xfe;

/// Cantidad de bytes inmediatos que lleva un opcode (sólo PUSHn).
pub fn bytes_push(op: u8) -> usize {
    if (PUSH1..=PUSH32).contains(&op) {
        (op - PUSH1 + 1) as usize
    } else {
        0
    }
}

/// Gas fijo de cada opcode (`None` = no soportado). Los costos dinámicos
/// (memoria, EXP, KECCAK, SSTORE, LOG) se cobran aparte en el intérprete.
pub fn gas_base(op: u8) -> Option<u64> {
    Some(match op {
        STOP | RETURN | REVERT => 0,
        JUMPDEST => 1,
        CALLER | CALLVALUE | CALLDATASIZE | POP | PC | MSIZE | GAS | PUSH0 => 2,
        ADD | SUB | LT | GT | EQ | ISZERO | AND | OR | XOR | NOT | BYTE | SHL | SHR | CALLDATALOAD | MLOAD | MSTORE | MSTORE8 => 3,
        PUSH1..=PUSH32 | DUP1..=DUP16 | SWAP1..=SWAP16 => 3,
        MUL | DIV | MOD => 5,
        JUMP => 8,
        EXP | JUMPI => 10,
        KECCAK256 => 30,
        SLOAD => 800,
        SSTORE => 0, // se cobra según el valor previo
        LOG0..=LOG4 => 375,
        _ => return None,
    })
}

/// Mnemónico → opcode (para el ensamblador).
pub fn opcode_de(m: &str) -> Option<u8> {
    let m = m.to_ascii_uppercase();
    let fijo = match m.as_str() {
        "STOP" => STOP, "ADD" => ADD, "MUL" => MUL, "SUB" => SUB, "DIV" => DIV, "MOD" => MOD, "EXP" => EXP,
        "LT" => LT, "GT" => GT, "EQ" => EQ, "ISZERO" => ISZERO, "AND" => AND, "OR" => OR, "XOR" => XOR,
        "NOT" => NOT, "BYTE" => BYTE, "SHL" => SHL, "SHR" => SHR, "KECCAK256" | "SHA3" => KECCAK256,
        "CALLER" => CALLER, "CALLVALUE" => CALLVALUE, "CALLDATALOAD" => CALLDATALOAD, "CALLDATASIZE" => CALLDATASIZE,
        "POP" => POP, "MLOAD" => MLOAD, "MSTORE" => MSTORE, "MSTORE8" => MSTORE8, "SLOAD" => SLOAD, "SSTORE" => SSTORE,
        "JUMP" => JUMP, "JUMPI" => JUMPI, "PC" => PC, "MSIZE" => MSIZE, "GAS" => GAS, "JUMPDEST" => JUMPDEST,
        "PUSH0" => PUSH0, "RETURN" => RETURN, "REVERT" => REVERT, "INVALID" => INVALID,
        _ => 0xff,
    };
    if fijo != 0xff {
        return Some(fijo);
    }
    let num = |p: &str| m.strip_prefix(p).and_then(|n| n.parse::<u8>().ok());
    match (num("PUSH"), num("DUP"), num("SWAP"), num("LOG")) {
        (Some(n @ 1..=32), ..) => Some(PUSH1 + n - 1),
        (_, Some(n @ 1..=16), ..) => Some(DUP1 + n - 1),
        (_, _, Some(n @ 1..=16), _) => Some(SWAP1 + n - 1),
        (_, _, _, Some(n @ 0..=4)) => Some(LOG0 + n),
        _ => None,
    }
}
