# 62 · Balanceador L4 TCP

Round-robin y least-connections, health-check por connect, relay async con tokio.

## Limitaciones
Health-check es connect (no HTTP), sin sticky sessions, sin pesos.
