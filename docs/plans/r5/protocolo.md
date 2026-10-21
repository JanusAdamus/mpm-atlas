# Ronda 5 · protocolo de medición de conexiones

Fecha: 2026-10-19.  Lo escribe el trabajador de conexiones (paso 2a), **antes de tocar datos y sin haber visto ningún resultado de la ronda 5**.  Plan: `docs/plans/2026-10-19-ronda-5-calidad-y-fuentes.md`, punto 0.  Cualquier cambio a este protocolo se anota abajo como enmienda fechada, antes de ver resultados.

## 1. Metas (copiadas tal cual del plan, «Reglas de decisión»)

| Componente | Pasa si |
|---|---|
| Aristas agregadas | 0 aristas concepto→concepto con estado más fuerte que su eslabón más débil (prueba) |
| Equivalencias | 0 composiciones transitivas de P460; las relaciones en los dos sentidos quedan en `disputed` |
| Muestra nueva de 20 conexiones de la capa general | ≥ 14/20 correctas y útiles, con dos jueces y acuerdo reportado |
| Sin regresión | pruebas en verde, AtlasBench sin regresión y diseño de la ronda 4 sin regresión |

Pruebas del ADR-0005 («Conexiones», sobre el registro después de M2), también como meta: 0 `equivalent_to` vigentes desde P460; 0 `special_case_of` vigentes desde P31; 0 `part_of_area` en `formally_linked`; todos los pares en los dos sentidos en `disputed`; volver a correr no cambia nada; AtlasBench sin regresión.

## 2. La muestra nueva de 20 conexiones

- **Semilla 20261020** (nueva; la 20261019 ya la usó el diagnóstico, §2.3).  Estrato directo con `Random(20261020)`, estrato agregado con `Random(20261021)`.
- **Capa general** = todos los nodos únicos de las teselas `layer=structure` z0–z2 del snapshot **final de la ronda** (el que publique el integrador), igual que el diagnóstico (§0).
- **Estratos** (mismo diseño que el diagnóstico, para comparar antes/después):
  - 10 afirmaciones vigentes del registro con dos extremos `concept` distintos en la capa;
  - 10 aristas agregadas concepto→concepto de `/api/concept` (`edge_limit=200`) con los dos extremos `concept` en la capa.
- **Cómo se saca**: sobre el servidor con el snapshot final, en `docs/plans/r5/consultas/` y con `python -I`: `r5_fetch_tiles.py` → `r5_load.py` → `r5_edges.py` → `r5_protocol_sample.py`.  El sorteo es función pura de la semilla y de los pools; `r5-protocol-sample.json` guarda la semilla, el tamaño de cada pool y los 20 ítems.  El `layout_version` y el `event_seq` del registro se anotan con el resultado.
- La muestra no se saca hoy a propósito: las operaciones M2 reemplazan afirmaciones (P31 y P460 con `supersede`), así que una muestra del registro de hoy mediría conexiones que dejarán de existir.

## 3. Cómo se juzga

- **Dos jueces independientes** (dos agentes distintos, p. ej. `data-scientist` y `computational-mathematician`, o Adrián y un agente).  Ninguno ve el juicio del otro ni el resultado de la ronda antes de terminar.
- Cada juez ve, por ítem: etiquetas ES/EN y descripciones de los dos extremos, la relación con su texto de `chain_authorizes`, el estado de evidencia mostrado, la marca «candidata» si la tiene y la afirmación testigo con su procedencia.  Puede consultar la fuente citada (Wikidata, Mathlib), nada más.
- **Dos preguntas por ítem**:
  1. ¿Correcta? `sí` / `dudosa` / `no`.  Correcta = lo que la pantalla afirma (relación + estado) es verdad con la lectura de `chain_authorizes`.
  2. ¿Útil para aprender? `sí` / `medio` / `no`.
- **Cuenta como «correcta y útil»** solo si el juez dice correcta = `sí` y útil ∈ {`sí`, `medio`}.
- **Veredicto del ítem** (conservador): pasa solo si **los dos jueces** lo dan por correcto y útil.  Se reportan también las cuentas de cada juez por separado.
- **Acuerdo**: acuerdo bruto y κ de Cohen sobre el resultado binario (pasa / no pasa), y la tabla 2×2.  Si κ < 0.4 se dice así y no se declara la meta cumplida aunque la cuenta llegue a 14.
- **Reporte**: cuenta sobre 20 con intervalo de Wilson al 95 %, y por estrato (directo y agregado, n = 10 cada uno, con su intervalo).  La meta se juzga sobre los 20.
- **Regla de decisión**: pasa si el veredicto conservador da ≥ 14/20 y κ ≥ 0.4.  Si no pasa, se reporta tal cual; no se cambia la muestra ni la regla.

## 4. Lo que no cuenta como evidencia

- El juicio de un solo agente (el diagnóstico de §2.3 fue de un solo juez: es la línea base, no la meta).
- Una muestra sacada de la mitad test de AtlasBench o con la semilla 20261019.
- Cifras sin la consulta que las produjo y la versión del snapshot.

## Enmiendas

(ninguna)
