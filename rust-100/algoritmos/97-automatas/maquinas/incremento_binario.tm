# Incremento binario: N -> N+1.
# Idea: ir al final del número, y desde allí sumar 1 con acarreo hacia la izquierda.
inicio: derecha
aceptar: fin

# 1) Avanzar hasta el primer blanco tras el número.
derecha 0 -> derecha 0 R
derecha 1 -> derecha 1 R
derecha _ -> acarreo _ L

# 2) Acarreo: un 1 se convierte en 0 y el acarreo sigue a la izquierda;
#    un 0 (o un blanco si todo eran unos) se convierte en 1 y terminamos.
acarreo 1 -> acarreo 0 L
acarreo 0 -> fin 1 S
acarreo _ -> fin 1 S
