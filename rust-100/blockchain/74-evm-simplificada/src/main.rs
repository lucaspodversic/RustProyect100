//! Demo: factorial(10) en bytecode y un contador persistente.
use evm_simplificada::asm::ensamblar;
use evm_simplificada::u256::U256;
use evm_simplificada::vm::{ejecutar, Entorno, Storage};

fn main() {
    let src = "
        PUSH1 1 PUSH1 10          ; acc=1, n=10   (pila: acc n)
      bucle:
        DUP1 ISZERO @fin JUMPI    ; si n==0 → fin
        DUP1 SWAP2 MUL SWAP1      ; acc*=n
        PUSH1 1 SWAP1 SUB         ; n-=1
        @bucle JUMP
      fin:
        POP PUSH0 MSTORE PUSH1 32 PUSH0 RETURN";
    let code = ensamblar(src).unwrap();
    println!("bytecode: 0x{}", hex::encode(&code));
    let env = Entorno { limite_gas: 100_000, ..Default::default() };
    let r = ejecutar(&code, &env, &mut Storage::new());
    println!("10! = {:?} ({} de gas) estado={:?}", U256::from_be_bytes(&r.salida).as_u64(), r.gas_usado, r.estado);
}
