# Castor afanoso (busy beaver) de 3 estados y 2 símbolos, campeón conocido:
# partiendo de la cinta en blanco escribe 6 unos y para tras 13 transiciones
# más la de parada (14 en total con este formato de "parada explícita").
# Se usa '0' como blanco para que la notación coincida con la literatura.
inicio: A
aceptar: H
blanco: 0

A 0 -> B 1 R
A 1 -> H 1 R
B 0 -> C 0 R
B 1 -> B 1 R
C 0 -> C 1 L
C 1 -> A 1 L
