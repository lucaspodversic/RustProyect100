# Palíndromos sobre {a,b}: borra el primer símbolo, recuerda cuál era
# (con el estado), corre al final y comprueba que el último coincide.
inicio: q0
aceptar: si
rechazar: no

q0 _ -> si _ S          # cadena vacía (o ya consumida): palíndromo
q0 a -> ira _ R         # recordamos 'a'
q0 b -> irb _ R         # recordamos 'b'

ira a -> ira a R
ira b -> ira b R
ira _ -> compa _ L      # al final: retrocede al último
irb a -> irb a R
irb b -> irb b R
irb _ -> compb _ L

compa a -> volver _ L   # coincide: lo borramos
compa b -> no b S
compa _ -> si _ S       # longitud impar: el central ya se borró
compb b -> volver _ L
compb a -> no a S
compb _ -> si _ S

volver a -> volver a L  # volver al principio y repetir
volver b -> volver b L
volver _ -> q0 _ R
