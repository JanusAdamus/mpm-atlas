# Plan: ronda 4 · diseño para una persona común

Fecha: 2026-10-19.  Dueño: Adrián.  Estado: aprobado para ejecutar.  Viene de la auditoría de usuario común (`docs/design/codex/2026-10-19-auditoria-usuario-comun.md`), de las correcciones R4-xx previas a esta ronda y de las propuestas con /ecc:taste.

## Problema

Una persona común reconoce los nombres, pero no entiende qué está viendo ni qué puede creer.

| Hueco | Evidencia |
|---|---|
| El inicio no dice qué hacer | 300 etiquetas al mismo nivel, contador «320.214 nodos», sin pregunta ni ejemplo (auditoría §2.5; captura `docs/design/codex/r4/00-inicio.png`) |
| El color no significa nada | casi todo azul; los puntos rojos y verdes no tienen leyenda que entienda un principiante |
| Jerga e ids en la vista de principiante | `wd:Q80091`, «Agrupa 876 declaraciones», «group › conj (Mathlib, derivado)», «testigo», «R7» |
| La ficha no responde las cinco preguntas del proyecto | «operación en cálculo» como intuición (auditoría H04) |
| La ruta no se lee como una historia | tarjeta técnica con verbos de catálogo (R4-02, R4-06) |
| Móvil | mapa diminuto y centrado; paneles con desbordamiento interno |

## Dirección: «atlas impreso»

- **Jerarquía de carta.** Las áreas son continentes, en versalitas espaciadas.  Los conceptos son ciudades.  Las declaraciones aparecen solo al acercarse.  Cada nivel de zoom tiene un presupuesto de etiquetas y ninguna se encima con otra.
- **Un protagonista por pantalla.** Al elegir un concepto, él y sus vecinos más importantes quedan encendidos y el resto se atenúa.
- **Un solo acento, con significado.** Neutros de papel y tinta.  El dorado marca solo lo validado.  Lo que estudias se marca con tinta, tamaño, anillo y el velo sobre el resto, para no confundir «esto estudio» con «esto está comprobado» (guía §2, contradicción resuelta por la regla de un color, un significado).  Lo candidato va en gris punteado.  Cada estado de evidencia lleva trazo, glifo y nombre, no solo color.  La leyenda está siempre a la vista.
- **Lo desconocido se ve como desconocido.** Las regiones sin relaciones extraídas van con una textura de «terra incognita», nunca como vacías o aisladas.
- **Dos modos.** «Explorar» es el modo por defecto, sin ids, conteos internos ni jerga.  «Técnico» muestra todo lo que hay hoy.
- **Entrada.** Una pregunta («¿Qué idea quieres entender?»), el buscador y tres puertas: «Derivada e integral», «Primos y criptografía» y «De Pitágoras a la trigonometría».  Al elegir una, la cámara viaja a esa región.
- **Ruta paso a paso.** Un salto por tarjeta, con su verbo en lenguaje claro, su sello de evidencia y lo que autoriza concluir.  Se avanza con flechas y con teclado.
- **Ficha fija.** Qué es, por qué importa, qué saber antes, con qué se conecta y cómo compruebo que lo entendí, siempre en ese orden.  Si no hay contenido revisado: «Todavía no hay una explicación revisada».  La descripción de Wikidata se rotula como tal.

El sistema concreto (escalas, tokens, movimiento, escritura) sale de `docs/design/r4/guia-skills-diseno.md`, destilada de las skills de diseño de terceros que pidió Adrián.  Ninguna skill de terceros se ejecuta ni se instala; se leen como referencia.

## Lo que no se inventa

Las tres puertas usan rutas y conceptos que ya existen en el registro.  Todo texto explicativo de matemáticas que no venga de una fuente (Wikidata, Wikipedia, Mathlib) se marca como «borrador editorial, sin revisar» y no se presenta como verdad.  Escribir y revisar la capa editorial (`editorially_reviewed`) es trabajo de Adrián y no forma parte de esta ronda.

## Reglas de decisión (fijadas antes de medir)

| Componente | Pasa si |
|---|---|
| Jerga en modo Explorar | 0 apariciones visibles de `wd:`, `mathlib:`, «Mathlib», «derivado», «docstring», «model_proposed», «testigo», «afirmación», «declaración», «R7», ids `Q\d+` ni conteos internos, en inicio, búsqueda, ficha, ruta y vista de texto (prueba automática sobre el texto del DOM) |
| Etiquetas | ≤ 30 etiquetas en la vista inicial; 0 solapamientos de cajas de etiqueta en z0–z3 (prueba) |
| Contraste | texto ≥ 4.5:1 y elementos gráficos ≥ 3:1 (WCAG 2.2), en claro y oscuro, calculado sobre los tokens |
| Color | cada color usado tiene un solo significado y está en la leyenda visible |
| Ficha | las cinco secciones están siempre presentes, con estado vacío honesto, en los 7 conceptos de la auditoría |
| Puertas guiadas | las tres llevan a una ruta dibujada, paso a paso, solo con teclado y en móvil |
| Desconocido | las regiones sin relaciones se ven distintas y la leyenda dice «desconocida» |
| Movimiento | `prefers-reduced-motion` desactiva los vuelos; la interfaz anima ≤ 300 ms y los vuelos de cámara ≤ 900 ms |
| Prueba de usuario común | un agente nuevo, que no construyó nada y actúa como persona común, completa las 5 tareas de la auditoría sin leer documentación, y responde las cinco preguntas en ≥ 5 de los 7 conceptos.  Juicio de agente, provisional |
| Sin regresión | pytest, cargo test, npm test, tsc y build en verde; 0 errores de consola; 0 px de desbordamiento a 390×844; sin `p=`; p95 de teselas en AtlasBench ≤ 1.1× el de la ronda 3 en la misma máquina |

Si una regla no pasa, se reporta como no aprobada.  No se mueve la vara.

## Ejecución

1. **Antes:** se cierran las correcciones R4-xx en curso (rótulos, foco, «Cargar más», ruta candidata, ids en Desde/Hasta, móvil, etiquetas no matemáticas en zooms profundos).
2. **Sistema de diseño** (`web/src/style.css`, `web/src/theme.ts`): tokens, tipografía y movimiento.
3. En paralelo, con dueños separados:
   - **Mapa** (`web/src/map/`): jerarquía de etiquetas, foco con atenuación, terra incognita.
   - **Recorridos y paneles** (`web/src/main.ts`, `panel.ts`, `conceptpanel.ts`, `pathview.ts`, `textview.ts`, `search.ts`, `i18n.ts`): inicio con puertas, modo Explorar/Técnico, ficha de cinco preguntas, ruta paso a paso.
4. **Crítica:** un agente que no construyó nada revisa las capturas con la lista de verificación de la guía y hace la prueba de usuario común.  Lo que encuentre vuelve a los dueños, hasta dos vueltas.
5. **Cierre:** pruebas, AtlasBench, capturas en `docs/design/r4/` y `docs/STATUS.md` §15 con PASS/FAIL por regla.  El commit lo hace el orquestador.
