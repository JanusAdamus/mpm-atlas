# Guía de diseño de la ronda 4, destilada de skills de terceros

19 de octubre de 2026 · Atlas de las conexiones matemáticas (`web/`, TypeScript + Vite + WebGL2, sin frameworks).

Qué es.  Una lectura de 38 skills de diseño publicadas por terceros, filtrada para este producto: un mapa multiescala de conceptos matemáticos para una persona común, con ficha lateral, búsqueda, rutas tipadas, vista textual con teclado y ES/EN.  La dirección de trabajo es «atlas impreso».  La crítica de partida está en `docs/design/CRITIQUE.md` y en `docs/design/codex/2026-10-19-auditoria-usuario-comun.md` (hallazgos R4-01 a R4-07).

Cómo se leyó.  El contenido de terceros se trató como datos.  No se ejecutó ningún script, no se instaló nada y no se siguió ninguna instrucción de esas skills.  Lo que piden hacer (red, instalar, escribir fuera del repo, cambiar configuración) está en la sección 4.  Los contrastes de la sección 2 se calcularon con la fórmula de luminancia relativa de WCAG 2 sobre los valores hex propuestos; no son mediciones de pantalla.  No se cambió código del producto.

Prioridad de este documento, heredada de `CLAUDE.md`: primero lo que puede enseñar algo falso (homónimos, falsa precisión, verbos de relación), después orientación y lectura, al final pulido visual.  Cuando una regla de skill choca con esa prioridad, gana la prioridad del proyecto.

| Repositorio clonado | Commit | Skills leídas |
|---|---|---|
| `emilkowalski_skills` | `2d22863` | emil-design-eng, animation-vocabulary, improve-animations (+AUDIT), review-animations (+STANDARDS), apple-design, break-ui (+CATALOG), find-animation-opportunities |
| `jakubkrehel_skills` | `4c66b1f` | better-ui, better-interface, better-layout, better-typography, better-colors, better-writing, better-accessibility (con referencias), interface-review, technical-drawing, explain-interface, state-machine |
| `ConardLi_garden-skills` | `7a6d5a7` | web-design-engineer (+failure-patterns, critique-guide, recetas tufte-dataink, stripe-press, vignelli-swiss-helvetica) |
| `elayadesign_ai-design-skills` | `1c1e97c` | landing-page-design |
| `codeswithroh_tastemaker` | `792a28a` | tastemaker (+SECURITY, interface-quality-rules) |
| `Owl-Listener_designer-skills` | `9a6930c` | design-critique, design-qa-checklist, journey-map, user-persona, usability-test-plan, motion-system, design-token, accessibility-audit, localization-design, theming-system |
| `MengTo_Skills` | `398cd41` | implement-fog-of-war (+mechanics), iterate-until-verified, optimize-web-animations, audit-verify-explain-grade-5, no-ai-design-slop, book-serif-index, light-mode-paper-technical |

Las rutas de cada skill son relativas a `scratchpad/ext-skills/` de la sesión.

---

## 1. Reglas por skill

Cada fila es una regla de la skill reescrita para el atlas.  «No aplica» resume en una línea lo que se descarta.

### 1.1 Emil Kowalski

**emil-design-eng** · `emilkowalski_skills/skills/emil-design-eng/SKILL.md`

| Regla | Cómo se aplica en el atlas |
|---|---|
| Decidir si algo anima según la frecuencia: 100+ veces al día, nunca; decenas, casi nada; ocasional, animación estándar. | Pan con flechas, `+`/`−`, `N`, foco de lista en la vista textual y hover de etiquetas: sin animación.  Abrir la ficha y dibujar una ruta nueva: animación estándar. |
| Nunca animar acciones iniciadas con teclado. | Las teclas del mapa mueven la cámara al instante.  Hoy la auditoría R4-04 muestra que el teclado ya es un camino central; no se le agrega retraso. |
| Entradas y salidas con `ease-out` fuerte (`cubic-bezier(0.23, 1, 0.32, 1)`), movimiento en pantalla con `cubic-bezier(0.77, 0, 0.175, 1)`; nunca `ease-in`. | Tokens `--ease-out` y `--ease-in-out` de la sección 2.5; la ficha entra con `--ease-out`, la cámara que viaja a un concepto usa `--ease-in-out`. |
| UI por debajo de 300 ms; salida más rápida que entrada. | Ficha: 200 ms al entrar, 150 ms al salir.  Ningún control de la cabecera pasa de 150 ms. |
| Animar solo `transform` y `opacity`; nunca `transition: all`. | La regla de `style.css:110` ya nombra propiedades; mantenerlo y aplicar lo mismo al panel de ruta. |
| Movimiento reducido es menos movimiento, no cero. | `style.css:372` hoy pone `transition: none !important` a todo; bajo `prefers-reduced-motion` debe quedar un fundido de opacidad de 150 ms en ficha y ruta para que se vea qué cambió. |

No aplica: resortes con `framer-motion`, gestos de arrastrar para descartar, toasts estilo Sonner y `clip-path` decorativo.

**animation-vocabulary** · `emilkowalski_skills/skills/animation-vocabulary/SKILL.md`

| Regla | Cómo se aplica en el atlas |
|---|---|
| Nombrar el efecto con el término exacto antes de pedirlo. | Las especificaciones de la ronda usan «continuity transition» para el viaje de cámara a un concepto, «line drawing» para trazar la ruta y «crossfade» para el cambio de ficha. |
| «Spatial consistency»: el elemento conserva su identidad entre estados. | El número de cada parada es el mismo en el mapa, en la lista de la ruta y en la vista textual. |
| «Tabular numbers» para cifras que cambian. | Contador de resultados y numeración de paradas con `font-variant-numeric: tabular-nums`. |

No aplica: el glosario de efectos ambientales (marquee, float, orbit, parallax) no tiene lugar en una herramienta de estudio.

**improve-animations** · `emilkowalski_skills/skills/improve-animations/{SKILL.md,AUDIT.md}`

| Regla | Cómo se aplica en el atlas |
|---|---|
| Primero mapear dónde vive el movimiento y su frecuencia; los planes extienden tokens existentes, no inventan paralelos. | Antes de tocar movimiento, inventariar `transition` en `style.css` y los bucles `requestAnimationFrame` de `web/src/map/`; los tokens nuevos sustituyen los `.12s` sueltos. |
| Severidad alta: easing equivocado, animación en teclado, cuadros perdidos. | En la revisión de la ronda, cualquier animación en un atajo del mapa es hallazgo alto. |
| Curvas y duraciones como tokens compartidos. | Un solo bloque `--dur-*` y `--ease-*` en `:root`, usado por CSS y leído por el motor de cámara en TypeScript. |

No aplica: el flujo de escribir planes en `plans/` y despachar ejecutores; aquí la orquestación la define `CLAUDE.md`.

**review-animations** · `emilkowalski_skills/skills/review-animations/{SKILL.md,STANDARDS.md}`

| Regla | Cómo se aplica en el atlas |
|---|---|
| Preferir borrar la animación antes que afinarla. | Ante la duda en mapa y búsqueda, quitar; el mapa ya se mueve por acción del usuario. |
| Transiciones interrumpibles, no `@keyframes`, para lo que se dispara seguido. | Atenuado de foco y vecinos con `transition` de opacidad, de modo que cambiar de concepto a media animación no reinicie. |
| Formato de hallazgo Antes / Después / Por qué. | La lista de verificación de la sección 3 pide ese formato para hallazgos de movimiento. |

No aplica: reglas de `framer-motion` y del origen de popovers anclados (el atlas no tiene popovers anclados).

**apple-design** · `emilkowalski_skills/skills/apple-design/SKILL.md`

| Regla | Cómo se aplica en el atlas |
|---|---|
| Responder en `pointerdown` y seguir el puntero 1:1 respetando el punto de agarre. | Arrastrar el mapa mueve el lienzo exactamente con el puntero; el zoom con rueda se ancla en el cursor. |
| Interrumpible siempre: animar desde el valor presente, nunca bloquear la entrada. | Si el usuario arrastra durante un viaje de cámara, el viaje se cancela y el mapa sigue al puntero desde donde estaba. |
| Proyección de inercia: `actual + (v/1000)·d/(1−d)`, `d ≈ 0.998`; bordes con resistencia, no tope duro. | Al soltar un arrastre rápido, el mapa desliza con esa fórmula; en el zoom mínimo y máximo hay resistencia progresiva en lugar de parar en seco. |
| Orientación: cada pantalla responde dónde estoy, a dónde puedo ir y cómo salgo. | La cabecera muestra la ruta de lectura «Área › concepto»; «Volver al panorama» está siempre visible con ficha o ruta abiertas. |
| Tracking por tamaño: negativo en grande, positivo en pequeño. | Título de ficha a 24 px con `−0.01em`; versalitas de área con tracking positivo (sección 2.2). |
| Familiaridad: lo que se ve igual se comporta igual. | Todo control que lleva al código matemático usa la misma etiqueta y el mismo glifo; nunca se disfraza de «Ver concepto» (R4-06, R4-07). |

No aplica: materiales translúcidos con `backdrop-filter`, háptica y sonido.  En un atlas impreso el papel es opaco.

**break-ui** · `emilkowalski_skills/skills/break-ui/{SKILL.md,CATALOG.md}`

| Regla | Cómo se aplica en el atlas |
|---|---|
| Probar con el peor caso realista, no con texto basura. | Fichas con nombres reales largos («hasFDerivAt_integral_of_dominated_of_fderiv_le''», «teorema fundamental del cálculo»), alias con diacríticos y 0, 1 y 200+ conexiones. |
| Plurales con `Intl.PluralRules`; números con `Intl.NumberFormat`. | «1 conexión», «2 conexiones», «318.733 nodos» en ES y «318,733 nodes» en EN. |
| Revisar a 320 px, al 200 % de zoom y en oscuro. | La auditoría ya midió desbordes internos a 390 px (panel de Pitágoras: 390 de ancho, 696 de contenido); se repite a 320 px. |
| Truncar con camino de vuelta; truncar al medio cuando lo que difiere está al final. | Identificadores técnicos largos en modo técnico: truncado al medio con el valor completo en un elemento enfocable. |

No aplica: el conmutador «Demo / Worst case» dentro del producto; aquí los peores casos van a fixtures de pruebas.

**find-animation-opportunities** · `emilkowalski_skills/skills/find-animation-opportunities/SKILL.md`

| Regla | Cómo se aplica en el atlas |
|---|---|
| Cada candidato pasa cuatro filtros: frecuencia, propósito, presupuesto de tiempo, función. | Sobreviven tres: entrada de la ficha (continuidad), trazado de una ruta nueva (explicación) y el viaje de cámara desde un resultado de búsqueda (orientación). |
| Datos que el usuario lee no se mueven por estilo. | Las etiquetas del mapa no flotan, no laten y no entran con escalonado. |
| Reportar también lo rechazado. | Rechazados: animar el contador de nodos, animar el cambio de idioma, animar resultados de búsqueda al teclear. |

No aplica: presupuesto de «deleite»; el único momento raro y emotivo es la pregunta de entrada, y se resuelve con tipografía, no con movimiento.

### 1.2 Jakub Krehel

**better-ui** · `jakubkrehel_skills/skills/better-ui/SKILL.md` (+ animations, enter-exit, icons, surfaces, performance)

| Regla | Cómo se aplica en el atlas |
|---|---|
| Presión a `scale(0.96)` en 150 ms `ease-out`; nunca en botones deshabilitados. | Botones «Buscar ruta», «Ver índice de conceptos» y los tres recorridos guiados. |
| Interacciones de alta frecuencia sin animación o ≤150 ms en `opacity`/`background-color`; toda animación deja una señal estática. | Hover de resultados de búsqueda: cambio de fondo en 100 ms.  El foco en el mapa se marca con anillo y peso de etiqueta, no solo con el atenuado. |
| Salidas más cortas y pequeñas que entradas. | Ficha: entra con 12 px de desplazamiento en 200 ms, sale con 150 ms. |
| Suprimir transiciones al cambiar de tema. | Al pulsar el conmutador de tema en `theme.ts`, desactivar transiciones un cuadro para que no se manche todo el panel. |
| Sombras para elevación, bordes para estructura; `border: 1px solid transparent` bajo sombras para modo de alto contraste. | Solo la ficha flotante sobre el mapa lleva sombra; el resto del papel se separa con filetes. |
| Iconos con `currentColor`, contorno por defecto, relleno para activo. | El glifo de cada estado de evidencia es un SVG con `currentColor`; ninguno lleva color fijo dentro del SVG. |

No aplica: recetas de Tailwind y de `motion/react`; el radio concéntrico importa poco con la gramática de radios casi rectos de la sección 2.4.

**better-interface** · `jakubkrehel_skills/skills/better-interface/{SKILL.md,review-format.md}`

| Regla | Cómo se aplica en el atlas |
|---|---|
| Revisar en este orden: accesibilidad, layout, escritura, tipografía, color, pulido. | Orden de la lista de verificación de la sección 3, compatible con la prioridad de `CLAUDE.md`. |
| Disparadores que son ALTO de inmediato: control sin nombre accesible, foco invisible, camino solo con puntero, recorte a 320 px o 200 %, significado solo por color, error sin salida, contenido oculto sin señal. | R4-03 (índice textual a 2240 px sin aviso) y R4-04 (foco que se queda en el lienzo) son disparadores de esta lista: bloquean el cierre de la ronda. |
| Preferir el arreglo más barato: borrar, usar la plataforma, reutilizar, corregir el valor, agregar. | Antes de agregar una nueva leyenda de evidencia, quitar la escala «Fuerza 5/5» del modo principiante (R4-02). |
| Sin evidencia renderizada, «No verificado». | Ningún hallazgo visual de la ronda se da por resuelto sin captura. |

No aplica: el tope de 15 hallazgos como cuota; la auditoría del 19 ya fijó el alcance.

**better-layout** · `jakubkrehel_skills/skills/better-layout/SKILL.md` (+ grouping-and-alignment, spacing-and-adaptivity)

| Regla | Cómo se aplica en el atlas |
|---|---|
| Agrupar con espacio; entre grupos al menos el doble que dentro. | En la ficha, 8 px dentro de cada una de las cinco secciones y 24 px entre secciones. |
| Una acción primaria por vista. | Entrada: «Explorar» con un recorrido; ficha: «Ver con qué se conecta»; ruta: «Seguir paso a paso».  El resto, secundario. |
| Toda parte oculta tiene una señal visible; un truncado tiene camino al texto completo. | R4-03: al activar la vista textual, el índice aparece primero y enfocado.  R4-05: «Estas son las primeras 200 conexiones» con «Cargar más conexiones». |
| Orden visual igual al orden del DOM; propiedades lógicas. | En móvil la vista textual va antes del formulario de ruta en el DOM, no reordenada con CSS. |
| Planear crecimiento por traducción: `min-height`, nunca altura fija; probar con el idioma más largo. | Cada botón se prueba en EN y ES; las etiquetas en español son en promedio más largas. |
| `100dvh` en paneles de altura completa en móvil. | La ficha en móvil como hoja inferior con `max-height: 100dvh` y su acción fuera de la zona con scroll. |

No aplica: reglas de RTL más allá de usar propiedades lógicas; ES y EN son LTR.

**better-typography** · `jakubkrehel_skills/skills/better-typography/SKILL.md` (+ spacing-and-sizing, wrapping-and-punctuation)

| Regla | Cómo se aplica en el atlas |
|---|---|
| Escala corta con nombre por rol; cada tamaño trae su interlineado y peso. | Escala de la sección 2.2: caption, ui, body, lead, title, display. |
| Interlineado sin unidades: display 1.1, títulos 1.2–1.3, cuerpo 1.5–1.6. | Ficha en serif a 16 px / 1.55. |
| Mínimo 12 px; captions 13 px; texto largo 16 px; tamaños en `rem`. | `--fs-xs: 11px` de `style.css:10` baja del piso; sube a 12 px como mínimo absoluto y 13 px por defecto. |
| Medida de 60–75 caracteres. | El cuerpo de la ficha se limita a `62ch`. |
| Puntuación tipográfica; texto en caso natural y `text-transform` para versalitas. | Comillas «» en ES y “” en EN, puntos suspensivos «…», raya para incisos.  Los nombres de áreas se guardan en caso natural y el mapa los pone en versalitas. |
| `tabular-nums` en valores que cambian; inputs a 16 px en móvil. | Contadores y numeración de paradas; el buscador a 16 px en móvil para que iOS no amplíe la página. |
| `lang` correcto y `translate="no"` en identificadores. | `lang="es"` o `lang="en"` en `<html>` según el idioma activo; nombres de declaraciones de Mathlib con `translate="no"`. |

No aplica: carga de fuentes de pago o variables; el proyecto usa pilas del sistema.

**better-colors** · `jakubkrehel_skills/skills/better-colors/SKILL.md` (+ contrast, palette-structure, token-naming, palette-generation)

| Regla | Cómo se aplica en el atlas |
|---|---|
| Un color, un significado; nada a menos de 15° de tono OKLCH del acento con otro significado. | Dorado solo para «validado».  El sepia de «histórico» (`#7a5a2a`) y el ámbar de «propuesta por modelo» quedan a menos de 15° del dorado y se retiran (sección 2.3). |
| Dos niveles de tokens: primitivos por tono, semánticos por rol; los componentes solo usan semánticos. | `--color-text-primary`, `--color-accent-solid`, `--color-evidence-validated`; el motor WebGL lee los semánticos ya resueltos desde `getComputedStyle`. |
| Medir el par renderizado; corregir la luminosidad antes que el tono. | Contrastes calculados de la sección 2.3; el dorado de texto se oscureció en luminosidad hasta pasar 4.5:1. |
| El modo oscuro no es el claro invertido: bajar croma, separar más los oscuros, volver a medir. | Dorado oscuro con croma similar pero más luminoso; filetes oscuros más separados. |
| Un solo mecanismo de cambio de tema. | `theme.ts` ya alterna; los tokens se definen una vez por `[data-theme]` con `prefers-color-scheme` solo como valor inicial. |
| Nunca el color como único portador del significado. | Cada estado de evidencia tiene trazo, glifo y nombre, además de color. |

No aplica: rampas completas de 11 o 12 pasos; el atlas necesita un neutro de papel, un acento y un estado de peligro.

**better-writing** · `jakubkrehel_skills/skills/better-writing/{SKILL.md,patterns.md}`

| Regla | Cómo se aplica en el atlas |
|---|---|
| Inventariar términos primero; una voz y un vocabulario. | Un glosario fijo en `i18n.ts`: «concepto», «conexión», «ruta», «paso», «respaldo técnico».  Prohibido llamar «concepto» a la agrupación técnica de declaraciones (R4-07). |
| Botones que empiezan con verbo. | «Buscar ruta», «Ver índice de conceptos», «Volver al mapa», «Cargar más conexiones». |
| Errores que dicen cómo arreglarlo, sin culpa ni «Ups». | «No se pudo cargar la ficha.  Revisa tu conexión e inténtalo de nuevo.» |
| Estados vacíos que apuntan hacia adelante. | «Sin resultados para «xyz».  Prueba con «derivada» o empieza un recorrido guiado.» |
| Frases completas con marcadores; plurales con reglas del idioma. | Mensajes ICU en `i18n.ts` en lugar de concatenar; «{n, plural, one {# conexión} other {# conexiones}}». |
| Pares de estado paralelos: «Buscando…» resuelve en «12 resultados». | Región `role="status"` del buscador. |

No aplica: confirmaciones destructivas y deshacer; el atlas no borra datos del usuario.

**better-accessibility** · `jakubkrehel_skills/skills/better-accessibility/SKILL.md` (+ focus-and-keyboard, motion-and-zoom, screen-readers)

| Regla | Cómo se aplica en el atlas |
|---|---|
| Todo camino con puntero tiene camino con teclado; Escape cierra lo último abierto. | Mapa, ficha, ruta y vista textual operables solo con Tab, flechas, Enter y Escape; Escape cierra ficha, luego ruta. |
| Al cambiar de vista, mover el foco al encabezado de la nueva vista y restaurarlo al volver. | R4-04: «Ver índice de conceptos» enfoca el encabezado del índice; «Volver al mapa» devuelve el foco al lienzo. |
| Anillo de foco visible con 3:1 contra todo lo que toca. | El anillo de tinta sobre papel mide 15.25:1 (sección 2.3); sobre la ficha oscura se usa el papel claro. |
| Anunciar cambios: `role="status"` estable para resultados y carga; `aria-describedby` para errores de campo. | Una sola región de estado, vacía desde el inicio, para «Buscando…», «12 resultados», «Ruta lista: 4 pasos». |
| Reflujo a 320 px y texto al 200 %; nunca `maximum-scale=1`. | El mapa es contenido 2D y puede tener scroll propio; la ficha y la vista textual no. |
| Objetivo mínimo 24×24 (AA); 44×44 en táctil. | Controles de zoom y cierre de ficha a 44 px en móvil. |

No aplica: formularios con `autocomplete` y validación de envío; el atlas solo tiene buscador y selector de ruta.

**interface-review** · `jakubkrehel_skills/skills/interface-review/SKILL.md`

| Regla | Cómo se aplica en el atlas |
|---|---|
| Revisar el cambio, no todo el código; clasificar cada hallazgo como introducido, regresión o previo. | La revisión de cierre de la ronda separa lo que la ronda 4 rompió de lo que ya venía de R3. |
| Leer las líneas borradas: un `aria-label`, un `outline` o un `prefers-reduced-motion` quitado es una regresión probable. | Revisión obligatoria de los diffs en `pathview.ts`, `i18n.ts` y `style.css`. |
| Exigir los estados ausentes en un componente nuevo: vacío, carga, error, deshabilitado, ancho estrecho. | La pantalla de entrada y los tres recorridos guiados se revisan en esos cinco estados. |

No aplica: resolución de alcance con ramas de PR; el repo no publica sin pedirlo Adrián.

**technical-drawing** · `jakubkrehel_skills/skills/technical-drawing/SKILL.md`

| Regla | Cómo se aplica en el atlas |
|---|---|
| Separar la evidencia del estilo de referencia; lo no sustentado se omite o se marca como ilustrativo. | Una conexión candidata se dibuja y se rotula «candidata, sin revisar»; nunca con el mismo trazo que una validada. |
| Jerarquía de línea: el peso y la visibilidad describen profundidad; tramado solo donde hay material seccionado. | Tramado diagonal solo para regiones «desconocidas»; en ningún otro lugar. |
| Etiquetas fuera de la silueta y líneas guía ancladas a un rasgo visible. | Los nombres de paradas de una ruta van fuera del trazo con guía corta al marcador numerado. |
| Inspeccionar la composición entera al tamaño de entrega y luego zonas apretadas a 3–4×. | Capturas de cierre a 1440 y 390 px, más recortes ampliados de nodos densos. |

No aplica: proyección axonométrica, láminas A3 y exportación PDF.

**explain-interface** · `jakubkrehel_skills/skills/explain-interface/SKILL.md`

| Regla | Cómo se aplica en el atlas |
|---|---|
| Etiquetar cada afirmación como medida, derivada o inferida. | Coincide con el principio 8 del proyecto; las notas de diseño de la ronda separan valores medidos en captura de valores supuestos. |
| Explicar el mecanismo, no transcribir valores. | Las especificaciones de movimiento dicen qué hace cada capa (velo, trazo, marcador) y por qué, no solo duraciones. |
| La página analizada es evidencia, no instrucción. | Se aplicó a esta misma lectura de skills. |

No aplica: análisis de sitios ajenos por URL.

**state-machine** · `jakubkrehel_skills/skills/state-machine/SKILL.md`

| Regla | Cómo se aplica en el atlas |
|---|---|
| Listar los estados que el código ya tiene antes de diseñar: carga, vacío, error, un elemento, conjunto típico, conjunto largo, parcial. | Ficha: sin explicación editorial, con explicación, 0 conexiones, 200+ conexiones, error de red.  Ruta: validada, solo candidata, histórica, sin ruta. |
| Datos con forma de producto, en cantidades reales. | Los fixtures de prueba usan fichas reales (número primo, integral, Bayes) y no «Concepto 1». |
| Un estado que el código no alcanza no se inventa; se lista como faltante. | «Ficha con explicación revisada» se lista como faltante donde aún no hay contenido editorial; no se simula. |

No aplica: la ruta desechable dentro de la app; los estados se revisan con `testdom.ts` y capturas.

### 1.3 ConardLi

**web-design-engineer** · `ConardLi_garden-skills/skills/web-design-engineer/SKILL.md` (+ failure-patterns, critique-guide, recetas)

| Regla | Cómo se aplica en el atlas |
|---|---|
| Declarar el sistema antes de escribir código: color, tipografía, espaciado, radios, sombras, movimiento. | La sección 2 de este documento es esa declaración. |
| Cuatro preguntas por pantalla: papel narrativo, distancia de lectura, temperatura visual, capacidad. | Entrada: invitación, 60 cm, cálida y tranquila, una pregunta y tres recorridos.  Ficha: lectura, 60 cm, sobria, cinco secciones.  Ruta: explicación, sobria, 3 a 7 pasos. |
| Prueba del borrado: si quitar un elemento no empeora el diseño, se quita. | Contador «318.733 nodos» fuera de la vista principal del principiante; queda en el modo técnico. |
| Nada de cifras inventadas; un hueco honesto vale más que un relleno. | «Aún no hay una explicación revisada de esta idea» en lugar de rellenar «Por qué importa» con texto genérico. |
| Receta Tufte: papel cálido, tinta, color solo con significado, etiquetas directas en lugar de leyendas. | Las paradas se rotulan sobre el mapa; la leyenda de evidencia se reduce a los estados visibles en pantalla. |
| Componentes con estados completos: vacío, carga, error, foco, deshabilitado. | Sección 2.6. |

No aplica: «Aim to stun», proporción 4–6× entre título y cuerpo, panel «Tweaks», variantes múltiples, CDN de React/Babel, protocolo de marca.

### 1.4 elayadesign

**landing-page-design** · `elayadesign_ai-design-skills/skills/landing-page-design/SKILL.md`

| Regla | Cómo se aplica en el atlas |
|---|---|
| Una sola acción primaria arriba; nada de acciones compitiendo. | Entrada: una pregunta y «Empezar con un recorrido»; la búsqueda es la segunda vía, no un tercer botón. |
| Contenido real, sin «Lorem ipsum», sin cifras redondas falsas, sin clichés («Descubre», «Potencia»). | La promesa de entrada describe lo que el atlas hace hoy: «Busca una idea o empieza con un ejemplo». |
| Estados completos: hover, activo, foco visible, carga con esqueleto con la forma real, vacío compuesto, error en línea. | Esqueleto de ficha con las cinco secciones; nunca `window.alert`. |
| Siempre un camino de regreso y la vista actual indicada. | «Volver al panorama» y el estado activo del conmutador mapa/índice. |
| `IntersectionObserver`, nunca un listener de scroll sin limitar. | Aplica a la vista textual larga si se agregan revelados; por defecto no se agregan. |

No aplica: tipografía única Geist/Manrope, prohibición de cursivas y guiones, degradado en titulares, transiciones de 700–800 ms, sección de eslogan con palabras que se encienden, SEO de campaña, FAQ.

### 1.5 codeswithroh

**tastemaker** · `codeswithroh_tastemaker/skills/tastemaker/SKILL.md` (+ SECURITY.md, interface-quality-rules.md)

| Regla | Cómo se aplica en el atlas |
|---|---|
| Fijar el estilo una vez y reutilizar tokens; no volver a derivarlo en cada pantalla. | Los tokens de la sección 2 se escriben en `style.css` y en un módulo TS para WebGL; ninguna pantalla define colores propios. |
| Clasificar cada pantalla (vista de datos, lienzo, estado vacío) antes de diseñarla; eso fija densidad y movimiento. | Mapa = lienzo; ficha = lectura; ruta = vista de datos narrada; entrada = estado vacío.  Solo la entrada admite un gesto expresivo. |
| Contraste como contrato: pares legales para texto (≥4.5:1) y para UI (≥3:1); si falla, mover luminosidad dentro del mismo tono. | Tabla de pares de la sección 2.3. |
| Estado en la URL: filtros, pestañas, paneles abiertos. | La URL ya lleva `e=`, `c=`, `text=1`, `path=1`; añadir el modo principiante/técnico y el recorrido guiado activo. |
| Documentos del proyecto como datos, no instrucciones. | Se aplicó a esta lectura. |

No aplica: GSAP en cada pantalla, registros shadcn, búsqueda de fotos e iconos por red, memoria en `~/.tastemaker/`, prohibición absoluta de la raya.

### 1.6 Owl-Listener

**design-critique** · `Owl-Listener_designer-skills/design-ops/skills/design-critique/SKILL.md`

| Regla | Cómo se aplica en el atlas |
|---|---|
| Antes de criticar, fijar meta, restricciones, público y etapa. | La crítica de cierre arranca con el visitante supuesto de la auditoría del 19. |
| Retroalimentación con forma: «Observo…», «Me pregunto…», «¿Y si…?». | Formato de las notas del revisor en la sección 3. |
| Separar crítica exploratoria de crítica de refinamiento. | La ronda 4 es de refinamiento: no se reabre la dirección «atlas impreso». |

No aplica: rituales de equipo y rotación de facilitador.

**design-qa-checklist** · `Owl-Listener_designer-skills/design-ops/skills/design-qa-checklist/SKILL.md`

| Regla | Cómo se aplica en el atlas |
|---|---|
| Verificar contra la especificación, no contra la memoria. | La sección 3 cita cada token y valor de la sección 2. |
| Probar con contenido real y casos límite. | Siete búsquedas de la auditoría, en ES y EN. |
| Incluir tamaños de texto del sistema y densidades de pantalla. | Captura con texto al 200 %. |

No aplica: flujo de bugs diseño contra implementación entre equipos.

**journey-map** · `Owl-Listener_designer-skills/design-research/skills/journey-map/SKILL.md`

| Regla | Cómo se aplica en el atlas |
|---|---|
| Etapas con meta, acción, punto de contacto, emoción, fricción y oportunidad. | El recorrido de cinco pasos de la auditoría (entender, encontrar, comprender, conectar, continuar) es el mapa de viaje de la ronda. |
| Señalar los momentos de verdad. | Dos: el primer resultado de búsqueda y la primera conclusión de una ruta.  Ahí se concentra el cuidado de redacción. |

No aplica: canales externos (correo, redes); el atlas tiene un solo punto de contacto.

**user-persona** · `Owl-Listener_designer-skills/design-research/skills/user-persona/SKILL.md`

| Regla | Cómo se aplica en el atlas |
|---|---|
| Personas por comportamiento, ancladas en datos; señalar huecos de investigación. | Persona primaria: estudiante o adulto curioso que conoce el nombre de una idea pero no Lean, Mathlib ni Wikidata.  Hueco: no hay entrevistas; la persona sale de la auditoría, no de usuarios reales. |
| Cada persona trae implicaciones de diseño. | Implicación: el modo principiante es el modo por defecto y nunca muestra identificadores. |

No aplica: fotos y nombres ficticios de personas.

**usability-test-plan** · `Owl-Listener_designer-skills/design-research/skills/usability-test-plan/SKILL.md`

| Regla | Cómo se aplica en el atlas |
|---|---|
| 3–5 preguntas de investigación, 5–8 tareas con criterio de éxito, 5–8 participantes. | Tareas: encontrar «integral» y decir qué es; explicar qué significa una conexión de la ruta derivada → integración; llegar al índice textual solo con teclado. |
| Métricas: éxito, tiempo, errores, SEQ. | Criterio de éxito de R4-02: el participante no interpreta la ruta candidata como demostrada. |
| Prueba piloto antes de las sesiones. | Una sesión piloto con alguien ajeno al proyecto. |

No aplica: reclutamiento formal y SUS con muestra grande.

**motion-system** · `Owl-Listener_designer-skills/design-systems/skills/motion-system/SKILL.md`

| Regla | Cómo se aplica en el atlas |
|---|---|
| 4–6 tokens de duración, curvas con nombre semántico. | Sección 2.5: cinco duraciones, tres curvas. |
| Escalonado total ≤500 ms; misma duración y curva por grupo semántico. | Las paradas de una ruta aparecen con 40 ms entre sí y tope de 300 ms. |
| Documentar también lo que no anima. | La sección 2.5 lista lo que nunca anima. |

No aplica: la sustitución global por 0 ms bajo movimiento reducido (sección 2.8, contradicción 5).

**design-token** · `Owl-Listener_designer-skills/design-systems/skills/design-token/SKILL.md`

| Regla | Cómo se aplica en el atlas |
|---|---|
| Tres niveles: global, alias, componente; los componentes nunca usan valores crudos. | Los `--st-*` de `style.css` se vuelven alias semánticos (`--color-evidence-*`) apuntando a primitivos. |
| Tokens de movimiento y elevación junto a color y espaciado. | `--dur-*`, `--ease-*` y `--shadow-float` en el mismo bloque `:root`. |

No aplica: exportación a Figma.

**accessibility-audit** · `Owl-Listener_designer-skills/design-systems/skills/accessibility-audit/SKILL.md`

| Regla | Cómo se aplica en el atlas |
|---|---|
| Hallazgos con criterio WCAG, severidad y remedio. | R4-03 → 2.4.3 orden del foco; R4-04 → 2.4.3 y 2.4.11; «Fuerza 5/5» sin explicación → 1.3.1/3.3.2 como recomendación. |
| Revisar estados estáticos e interactivos. | Captura de foco en cada control de la cabecera. |

No aplica: sesiones con tecnologías de asistencia reales en esta ronda; se marca como pendiente.

**localization-design** · `Owl-Listener_designer-skills/design-systems/skills/localization-design/SKILL.md`

| Regla | Cómo se aplica en el atlas |
|---|---|
| Contenedores flexibles; probar con la traducción más larga. | Cada etiqueta de la cabecera se mide en ES, que hoy es la más larga. |
| Fechas y números con `Intl`. | «318.733» en es-MX, «318,733» en en-US. |
| La notación matemática no se refleja ni se traduce. | Fórmulas con `translate="no"` y `dir="ltr"`. |
| Localizar significado, no solo palabras. | «Ruta» y «route», «conexión» y «connection» se fijan en el glosario bilingüe. |

No aplica: RTL y escrituras CJK.

**theming-system** · `Owl-Listener_designer-skills/design-systems/skills/theming-system/SKILL.md`

| Regla | Cómo se aplica en el atlas |
|---|---|
| Los temas sobreescriben solo el nivel semántico. | Claro, oscuro y alto contraste cambian alias, no componentes ni shaders. |
| En oscuro, superficies más claras para elevar, menos saturación. | Ficha oscura `#1c1a16` sobre papel oscuro `#14130f`; la elevación es la diferencia de superficie, no una sombra. |
| Respetar la preferencia del sistema. | `prefers-color-scheme` como valor inicial; el conmutador manda después. |

No aplica: multimarca.

### 1.7 MengTo

**implement-fog-of-war** · `MengTo_Skills/agent-skills/codex/implement-fog-of-war/{SKILL.md,references/mechanics.md}`

| Regla | Cómo se aplica en el atlas |
|---|---|
| Un solo modelo autoritativo de visibilidad; la presentación nunca decide la verdad. | La cobertura («desconocida» o con datos) sale del registro, no de cuántos píxeles tiene una zona. |
| Una sola capa de velo con opacidad máxima contenida; la zona central siempre legible. | El atenuado de «foco + vecinos» es un solo velo de papel; el resto del mapa baja a opacidad 0.35–0.45 y las etiquetas de área siguen ≥3:1. |
| Bordes suaves, sin rayos visibles; nunca oscurecer toda la escena para compensar. | El borde entre foco y resto se suaviza; el resto no se vuelve negro en tema oscuro. |
| Desactivar el velo en estados que no son de juego (menús, capturas, transiciones). | El velo de foco se apaga en la vista textual y en el panorama inicial. |

No aplica: rayos de línea de visión, enemigos y three.js.

**iterate-until-verified** · `MengTo_Skills/agent-skills/codex/iterate-until-verified/SKILL.md`

| Regla | Cómo se aplica en el atlas |
|---|---|
| Convertir «bonito» o «claro» en compuertas observables pasa/no pasa. | La sección 3 está escrita como compuertas binarias. |
| Separar quien hace de quien juzga. | El revisor de capturas no es quien implementó. |
| Terminar solo con todas las compuertas en verde o un bloqueo nombrado. | La ronda cierra con `docs/STATUS.md` diciendo qué compuerta quedó bloqueada y por qué. |

No aplica: abrir subagentes sin el rol que fija la tabla de orquestación de `CLAUDE.md`.

**optimize-web-animations** · `MengTo_Skills/agent-skills/codex/optimize-web-animations/SKILL.md`

| Regla | Cómo se aplica en el atlas |
|---|---|
| Medir antes de editar; `animation-play-state` no detiene un bucle `requestAnimationFrame`. | El bucle de render de `web/src/map/` se detiene cuando el lienzo no es visible (pestaña oculta o vista textual activa) y se reanuda al volver. |
| Limpiar observadores, temporizadores y recursos WebGL al desmontar. | Al cerrar la ruta, liberar buffers de la capa de ruta; al cambiar de tema, no recrear texturas que no cambian. |
| Limitar el delta de tiempo tras una pausa. | La inercia del mapa no salta al volver a la pestaña. |
| Reportar mediciones separadas de inferencias. | Coincide con el principio 8 del proyecto. |

No aplica: comandos de git con índice temporal y el uso del navegador de Codex.

**audit-verify-explain-grade-5** · `MengTo_Skills/agent-skills/codex/audit-verify-explain-grade-5/SKILL.md`

| Regla | Cómo se aplica en el atlas |
|---|---|
| Frases cortas; definir cada término técnico en palabras simples. | Modo principiante: «respaldo técnico (código matemático)» en lugar de «declaraciones»; «candidata» con una frase que dice qué significa. |
| No decir «todo funciona» si solo se probó una parte. | Mensajes de la ruta: «Conexión candidata; pendiente de revisión», nunca una certeza implícita. |
| Separar hecho de juicio. | La tarjeta de conclusión de ruta separa «Qué dice el registro» de «Qué no se ha comprobado». |

No aplica: el formato de informe de cambios para desarrolladores.

**no-ai-design-slop** · `MengTo_Skills/agent-skills/ui/no-ai-design-slop/SKILL.md`

| Regla | Cómo se aplica en el atlas |
|---|---|
| Un punto focal por vista y una tesis visual por superficie. | Tesis: «una página de atlas impreso en la que una idea está iluminada». |
| Jerarquía antes que etiquetas: tamaño, peso y posición antes de píldoras y rótulos. | Las insignias de evidencia en la ficha se reducen a glifo + palabra; sin píldoras de color. |
| Prueba de remoción: nombrar el elemento, decir su trabajo, quitarlo mentalmente. | Se aplica a cada rótulo en versalitas de la ficha y a cada línea de metadatos. |
| No inventar actividad, métricas ni pantallas. | Ninguna cifra de «conceptos explorados» ni progreso ficticio. |

No aplica: la auditoría de autoría por IA.

**book-serif-index** · `MengTo_Skills/agent-skills/web-design/book-serif-index/SKILL.md`

| Regla | Cómo se aplica en el atlas |
|---|---|
| Serif legible para leer, mono o versalitas para el índice y los metadatos. | Ficha en serif; índice textual con números de página-concepto en `tabular-nums`. |
| Navegación de índice con estado activo y marcadores pequeños. | Vista textual como índice de atlas: área en versalitas, conceptos debajo, el concepto activo marcado con un filete dorado solo si está validado. |
| Acento concentrado en estados activos y notas, nunca repartido. | El dorado no aparece en botones ni encabezados. |

No aplica: textura de papel envejecido, pliegue central y marco oscuro alrededor.

**light-mode-paper-technical** · `MengTo_Skills/agent-skills/web-design/light-mode-paper-technical/SKILL.md`

| Regla | Cómo se aplica en el atlas |
|---|---|
| Papel cálido en lugar de blanco puro; un acento que puntúa. | Fondo `#f4f1e8`, ficha `#fbfaf5`. |
| Estructura con filetes finos y escuadras discretas. | Marcas de esquina en los bordes del mapa como en una hoja impresa, solo en escritorio. |

No aplica: textura diagonal en superficies grandes y contenedor redondeado sobre fondo oscuro.

---

## 2. Síntesis: sistema de diseño de la ronda 4

Tesis visual (no-ai-design-slop): una página de atlas impreso en la que una idea está iluminada.  El papel y la tinta llevan el 90 % de la interfaz (better-colors, palette-structure); el color aparece solo cuando significa algo.

### 2.1 Reglas que gobiernan todo lo demás

| Regla | Origen |
|---|---|
| Lo que puede enseñar algo falso se arregla antes que lo demás: «Fuerza 5/5» fuera del modo principiante, candidata nunca con trazo de validada, «concepto» nunca para la agrupación técnica. | `CLAUDE.md`; better-interface (disparador «color usado contra su significado»); audit-verify-explain-grade-5 |
| Un objeto protagonista por pantalla: foco + vecinos a plena tinta, resto atenuado bajo un solo velo. | no-ai-design-slop; implement-fog-of-war |
| Un color, un significado. | better-colors |
| La región sin datos es «desconocida», con tramado y rótulo, nunca vacía ni «aislada». | `CLAUDE.md` principio 7; technical-drawing (tramado con significado) |
| Modo principiante por defecto, sin identificadores ni jerga; modo técnico aparte, con nombre propio y estado en la URL. | user-persona; audit-verify-explain-grade-5; tastemaker (estado en la URL) |
| Cada vista responde dónde estoy, a dónde puedo ir y cómo salgo. | apple-design (wayfinding); better-layout |

### 2.2 Tipografía

Familias.  Serif de libro para lectura y lettering del mapa (`--font-serif`, la pila actual Iowan/Charter/Sitka/Cambria/Georgia); sans del sistema para controles (`--font-ui`); mono solo en modo técnico.  Dos familias de lectura más una utilitaria, dentro del límite de tres (better-typography) y del de dos para lectura (critique-guide).  El sistema como fuente de UI es correcto (apple-design §15); la regla de ConardLi contra `system-ui` aplica solo a tipografía de identidad, que aquí es la serif.

Escala de interfaz (better-typography, spacing-and-sizing; Apple para tracking):

| Rol | Token | Tamaño | Interlineado | Peso | Tracking | Uso |
|---|---|---|---|---|---|---|
| caption | `--text-caption` | 0.8125rem (13 px) | 1.4 | 400 | 0 | Metadatos, procedencia, ayuda de teclado |
| label | `--text-label` | 0.75rem (12 px) | 1.3 | 600 | 0.05em, versalitas | Rótulos de las cinco secciones de la ficha.  Piso absoluto: nada por debajo de 12 px |
| ui | `--text-ui` | 0.875rem (14 px); 1rem en inputs móviles | 1.45 | 400 / 500 | 0 | Botones, buscador, resultados |
| body | `--text-body` | 1rem (16 px), serif | 1.55 | 400 | 0 | Cuerpo de ficha, vista textual; medida máxima `62ch` |
| lead | `--text-lead` | 1.125rem (18 px), serif | 1.45 | 400 | 0 | «Qué es», primera frase de la ficha; conclusión de ruta |
| title | `--text-title` | 1.5rem (24 px), serif | 1.2 | 600 | −0.01em | Nombre del concepto en la ficha |
| display | `--text-display` | 2.25rem (36 px), serif | 1.1 | 500 | −0.02em | Solo la pregunta de entrada |

Cambio concreto: `--fs-xs: 11px` y `--fs-sm: 12.5px` de `style.css` se sustituyen por `label` y `caption`.  `tabular-nums` en contadores, numeración de paradas y números del índice.  `text-wrap: balance` en títulos, `pretty` en párrafos.  Cursiva solo para nombres de fuentes y para declaraciones en modo técnico.

Escala del mapa (jerarquía de atlas impreso).  Tamaños en px CSS, multiplicados por `devicePixelRatio` en WebGL; todas las etiquetas con halo de papel de 3 px (`--halo`, ya existe):

| Nivel | Cuándo aparece | Estilo |
|---|---|---|
| Área (continente) | Panorama y zoom medio | Serif 600, `font-variant-caps: all-small-caps`, 15 px, tracking 0.14em, color `text-secondary`; a zoom alto baja a 13 px y se atenúa |
| Subárea | Zoom medio | Serif 500, versalitas, 13 px, tracking 0.08em |
| Concepto (ciudad) | Zoom medio y alto | Serif 400, 14 px, `text-primary`, punto de 4 px |
| Concepto en foco | Siempre que hay foco | Serif 600, 17 px, punto de 7 px con anillo de tinta |
| Vecino del foco | Con foco | Serif 500, 14 px, a plena tinta |
| Declaración | Solo modo técnico y zoom máximo | Sans 400 cursiva, 12 px, `text-muted`, `translate="no"` |

El tracking ancho de áreas (0.14em) contradice el 0.05em de better-typography para rótulos en mayúsculas; gana la convención cartográfica en el lienzo, porque el espaciado indica extensión territorial.  En la UI HTML se queda 0.05em.

### 2.3 Color

Primitivos en hex (la notación del proyecto; better-colors pide no mezclar notaciones).  Contraste WCAG 2 calculado con la fórmula de luminancia relativa sobre estos valores; el par se mide contra la superficie donde se pinta.

**Claro**

| Token semántico | Valor | Par medido | Contraste | Umbral |
|---|---|---|---|---|
| `--color-bg-page` (papel) | `#f4f1e8` | — | — | — |
| `--color-bg-surface` (ficha) | `#fbfaf5` | — | — | — |
| `--color-bg-raised` | `#ffffff` | — | — | — |
| `--color-text-primary` (tinta) | `#1c1b18` | sobre papel / ficha | 15.25 / 16.48 | 4.5 |
| `--color-text-secondary` | `#48443b` | sobre papel / ficha | 8.59 / 9.28 | 4.5 |
| `--color-text-muted` | `#655f53` | sobre papel / ficha | 5.61 / 6.06 | 4.5 |
| `--color-border` (filete) | `#d6cfbf` | sobre ficha | 1.48 | decorativo; nunca solo para un control |
| `--color-border-strong` (borde de control) | `#8f8775` | sobre papel / ficha | 3.16 / 3.41 | 3.0 |
| `--color-accent-text` (dorado tinta) | `#7a5600` | sobre papel / ficha / fondo dorado | 5.89 / 6.36 / 5.31 | 4.5 |
| `--color-accent-solid` (dorado trazo) | `#a87a12` | sobre papel / ficha | 3.41 / 3.68 | 3.0 (no texto) |
| `--color-accent-subtle` | `#f2e5c2` | tinta encima | 13.75 | 4.5 |
| `--color-candidate` (gris punteado) | `#857e70` | sobre papel / ficha | 3.57 / 3.85 | 3.0 (no texto) |
| `--color-danger` (disputada) | `#a3361f` | sobre papel / ficha / fondo | 5.99 / 6.47 / 5.43 | 4.5 |
| `--color-danger-subtle` | `#f7e2da` | — | — | — |
| `--color-unknown` (tramado) | `#e6e0d2` | tinta / atenuado encima | 13.09 / 4.81 | 4.5 |
| `--color-focus-ring` | = tinta | sobre papel | 15.25 | 3.0 |

**Oscuro**

| Token semántico | Valor | Par medido | Contraste | Umbral |
|---|---|---|---|---|
| `--color-bg-page` | `#14130f` | — | — | — |
| `--color-bg-surface` | `#1c1a16` | — | — | — |
| `--color-bg-raised` | `#252219` | tinta clara encima | 12.76 | 4.5 |
| `--color-text-primary` | `#ece6d6` | sobre papel / ficha | 14.92 / 13.95 | 4.5 |
| `--color-text-secondary` | `#c6bea9` | sobre papel / ficha | 10.04 / 9.38 | 4.5 |
| `--color-text-muted` | `#9c9483` | sobre papel / ficha | 6.18 / 5.77 | 4.5 |
| `--color-border` | `#38342b` | sobre ficha | 1.40 | decorativo |
| `--color-border-strong` | `#6e6757` | sobre papel / ficha | 3.31 / 3.09 | 3.0 |
| `--color-accent-text` | `#e3b74e` | sobre papel / ficha / fondo dorado | 9.87 / 9.22 / 6.98 | 4.5 |
| `--color-accent-solid` | `#c99a2e` | sobre papel / ficha | 7.21 / 6.74 | 3.0 |
| `--color-accent-subtle` | `#3a2f14` | tinta clara encima | 10.56 | 4.5 |
| `--color-candidate` | `#8a8374` | sobre papel / ficha | 4.94 / 4.62 | 3.0 |
| `--color-danger` | `#f08a6c` | sobre papel / ficha / fondo | 7.57 / 7.08 / 6.16 | 4.5 |
| `--color-danger-subtle` | `#3a1f17` | — | — | — |
| `--color-unknown` | `#24211b` | tinta / atenuado encima | 12.88 / 5.33 | 4.5 |
| `--color-focus-ring` | = tinta clara | sobre papel | 14.92 | 3.0 |

Tonos OKLCH calculados: dorado 80–86°, peligro 34–37°, separación >40°, por encima de los 15° que pide better-colors.  El gris candidato tiene croma 0.02, es decir, neutro.

Prohibido: texto blanco sobre `--color-accent-solid` claro (3.85:1) y tinta sobre el mismo fondo (4.48:1).  El dorado sólido es solo trazo, punto o filete; el texto dorado usa `--color-accent-text`.

Estados de evidencia: el color nunca va solo (better-accessibility 1.4.1).  Cada estado tiene trazo, glifo y nombre:

| Estado del registro | Nombre en modo principiante | Trazo en el mapa | Glifo | Color |
|---|---|---|---|---|
| `editorially_reviewed`, `formally_linked` | Validada | Continuo, 2 px | ✓ | `accent-solid` |
| `extracted` | Documentada, sin revisar | Continuo fino, 1 px | — | `text-secondary` |
| `model_proposed` (candidata) | Candidata, sin revisar | Punteado 1 px | ? | `candidate` |
| `disputed` | En disputa | Discontinuo 1.5 px | ! | `danger` |
| `superseded`, `retracted` | Retirada | Oculta en principiante; tachada en técnico | × | `text-muted` |
| Conexión histórica | Conexión histórica, no dependencia | Discontinuo largo de tinta | ⧗ | `text-primary` |
| Región sin datos | Desconocida | Tramado diagonal 45°, 1 px cada 6 px | — | `unknown` |

Cambios frente a `style.css` hoy: se retiran el azul de acción `#1f56b8`, el sepia de histórico `#7a5a2a` (hue a menos de 15° del dorado) y la paleta Okabe-Ito como color de estado.  Las acciones primarias se rellenan de tinta con texto papel; los enlaces son tinta subrayada (better-colors: un acento con un solo significado; book-serif-index: acento concentrado).

Alto contraste (`prefers-contrast: more`): filetes a `border-strong`, candidato a `text-muted`, velo de atenuado a la mitad.

### 2.4 Espaciado y forma

| Token | Valor | Uso | Origen |
|---|---|---|---|
| `--s1` | 4 px | Glifo con su palabra | escala actual |
| `--s2` | 8 px | Dentro de un grupo | better-layout |
| `--s3` | 12 px | Entre controles con borde | better-layout |
| `--s4` | 16 px | Margen lateral móvil; paso de anidado | better-layout |
| `--s5` | 24 px | Entre grupos; margen de ficha en escritorio; entre controles sin borde | better-layout |
| `--s6` | 32 px | Entre bloques mayores | escala actual |
| `--s7` | 48 px | Entrada: aire alrededor de la pregunta | nuevo |

Radios: 2 px en insignias, 4 px en controles, 0 en la ficha cuando está acoplada al borde y 6 px cuando flota.  Hoy `--radius` es 8 px; un atlas impreso pide esquinas casi rectas (receta vignelli/tufte: radio 0).  Sombra: una sola, `--shadow-float`, para la ficha flotante sobre el mapa; en oscuro, un anillo de 1 px en lugar de sombra (better-ui, surfaces).  Objetivos táctiles de 44 px y de escritorio de 32 px como mínimo, nunca menos de 24 px (better-accessibility).

### 2.5 Movimiento

Tokens (Emil para curvas y duraciones; Owl para la forma del sistema):

| Token | Valor | Uso |
|---|---|---|
| `--dur-instant` | 0 ms | Todo lo iniciado por teclas repetidas; hover de etiquetas del mapa |
| `--dur-fast` | 100 ms | Hover de botones y resultados (`background-color`, `ease`) |
| `--dur-press` | 150 ms | `scale(0.96)` al presionar |
| `--dur-panel-in` / `--dur-panel-out` | 200 ms / 150 ms | Ficha y panel de ruta: `opacity` + `translate` 12 px |
| `--dur-camera` | 300 ms | Viaje de cámara a un destino elegido (resultado de búsqueda, paso de recorrido guiado) |
| `--dur-route-draw` | 600 ms | Trazado de una ruta nueva, una vez; propósito explicativo |
| `--stagger` | 40 ms, tope 300 ms | Aparición de paradas en la lista de ruta |
| `--ease-out` | `cubic-bezier(0.23, 1, 0.32, 1)` | Entradas y salidas |
| `--ease-in-out` | `cubic-bezier(0.77, 0, 0.175, 1)` | Cámara que viaja |
| `--ease-color` | `ease` | Cambios de color |

Lo que nunca anima: flechas, `+`/`−`, `N`, cambio de idioma, cambio de tema (con supresión de transiciones), contador de nodos, resultados mientras se escribe, etiquetas del mapa.

Mapa con puntero: seguimiento 1:1, zoom anclado al cursor, inercia con `d = 0.998` y resistencia progresiva en los límites de zoom (apple-design §2, §6, §9).  Un arrastre durante el viaje de cámara cancela el viaje desde el valor presente.  El bucle de render se detiene con la pestaña oculta o la vista textual activa (optimize-web-animations).

Movimiento reducido: cámara instantánea, ruta completa sin trazado, paradas sin escalonado, ficha con fundido de opacidad de 150 ms.  Se reemplaza la regla global `transition: none !important` de `style.css:372` por tokens que bajo `reduce` conservan solo opacidad.

### 2.6 Estados vacío, carga, error y parcial

| Estado | Qué se ve | Texto (ES) | Origen |
|---|---|---|---|
| Entrada | Papel, pregunta en `display`, buscador, tres recorridos | «¿Cómo se conectan las ideas matemáticas?  Busca una idea o empieza con un ejemplo.»  Recorridos: «Derivada e integral», «Primos y criptografía», «Pitágoras y trigonometría» | better-writing (vacío que apunta adelante); landing-page-design (una acción); auditoría paso 1 |
| Carga del mapa | Papel con marcas de esquina; texto en `role="status"` | «Dibujando el mapa…» | better-writing (estado en curso); elaya (sin spinner) |
| Carga de ficha | Esqueleto con las cinco secciones en su posición | «Cargando la ficha de «integral»…» (solo lector de pantalla) | landing-page-design B9; better-accessibility |
| Búsqueda en curso / lista | Resultados conceptuales primero; técnicos en otro grupo | «Buscando «integral»…» → «12 resultados» | better-writing (pares paralelos) |
| Sin resultados | Mensaje y salida | «Sin resultados para «xyz».  Prueba con «derivada» o empieza un recorrido guiado.» | better-writing |
| Error de red | En línea, con acción | «No se pudo cargar la ficha.  Revisa tu conexión e inténtalo de nuevo.»  Botón «Reintentar» | better-writing; better-interface (error con salida) |
| Ficha sin explicación revisada | Hueco honesto | «Aún no hay una explicación revisada de esta idea.  Descripción breve: operación del cálculo.» | web-design-engineer (hueco honesto); auditoría §5 |
| Lista recortada | Conteo honesto y control | «Estas son las primeras 200 conexiones.»  Botón «Cargar más conexiones» | better-layout; auditoría R4-05 |
| Sin ruta validada | Separado del camino documental | «Todavía no hay una conexión validada entre estas ideas.  Esto no significa que no estén relacionadas.»  Acción secundaria: «Ver conexiones candidatas (sin revisar)» | auditoría R4-02; better-writing (sin «nosotros») |
| Ruta candidata | Encabezado en gris, trazo punteado | «Conexión candidata; pendiente de revisión.» | auditoría R4-02 |
| Región sin datos | Tramado y rótulo al pasar o enfocar | «Región desconocida: todavía no hay conexiones registradas aquí.» | `CLAUDE.md` principio 7 |

### 2.7 Escritura de la UI

- Una voz: directa, en segunda persona («Busca», «Prueba»); nunca «nosotros» ni «Ups»; sin signos de exclamación en errores (better-writing; elaya B8).
- Botones con verbo: «Buscar ruta», «Ver índice de conceptos», «Volver al mapa», «Volver a mi ruta», «Cargar más conexiones», «Ver respaldo técnico (código matemático)» (better-writing; auditoría §5).
- Glosario cerrado, ES/EN: concepto/concept, conexión/connection, ruta/route, paso/step, validada/validated, candidata/candidate, desconocida/unknown, respaldo técnico/technical backing.  «Declaración», «docstring», «testigo», «R7» y «score» solo en modo técnico.
- Verbo de relación escrito desde el sentido recorrido, en lenguaje llano: «Algunos resultados sobre integración usan resultados sobre derivadas», no «depende lógicamente de → recorrida al revés» (auditoría R4-02; audit-verify-explain-grade-5).
- Frases completas con marcadores ICU y plurales del idioma; números y fechas con `Intl` (better-writing; localization-design).
- Comillas «» en español y “” en inglés, «…» como un solo carácter, raya para incisos con moderación (better-typography).
- Caso de oración en botones y títulos (better-writing; elaya B8).
- Cero cifras sin explicación en modo principiante: si una métrica aparece, dice qué estima (principio 9 del proyecto).

### 2.8 Contradicciones entre skills y cuál gana

| # | Choque | Gana | Por qué |
|---|---|---|---|
| 1 | Emil: nunca animar acciones de teclado.  Apple: continuidad espacial al cambiar de lugar. | Emil para teclas repetidas (instantáneo); Apple para elegir un destino puntual (cámara 300 ms, con puntero o con Enter). | La frecuencia decide el costo; orientarse en el mapa tras elegir un resultado es parte del aprendizaje. |
| 2 | Escalonado: Jakub 100 ms; Emil 30–80 ms; Owl 30–50 ms con total ≤500 ms. | 40 ms con tope de 300 ms. | Con 3–7 paradas, 100 ms da hasta 700 ms; las tres reglas se cumplen solo en ese rango. |
| 3 | Presión: Jakub `0.96` / 150 ms exactos; Emil 0.95–0.98 / 100–160 ms. | `0.96` / 150 ms. | Cumple ambas. |
| 4 | Curvas: Emil `(0.23, 1, 0.32, 1)`; Owl y Jakub `(0.2, 0, 0, 1)`. | Emil. | Un solo vocabulario de curvas; Emil da curvas por propósito (entrada y movimiento). |
| 5 | Movimiento reducido: Owl, 0 ms global.  Emil, Jakub y Apple: menos, no cero; conservar opacidad. | Emil, Jakub y Apple. | El lector necesita ver qué cambió; la opacidad no es un disparador vestibular. |
| 6 | Elaya: una tipografía, sin cursivas, Geist o Manrope, sin guiones. | La dirección del producto (serif + sans, cursiva acotada). | Elaya se declara para landing pages y cede ante la instrucción explícita; un atlas necesita lettering serif. |
| 7 | Tastemaker: GSAP y movimiento en cada pantalla.  Emil y find-animation-opportunities: contención. | Contención. | `CLAUDE.md` pide web sin frameworks pesados y dependencias justificadas; el mapa es una herramienta de uso diario. |
| 8 | Tastemaker: nunca raya.  Jakub: puntuación tipográfica, raya incluida. | Jakub, con uso moderado. | La raya es norma del español para incisos. |
| 9 | ConardLi: «aim to stun», título 4–6× el cuerpo. | Contención; display 36 px contra cuerpo 16 px. | La propia guía de crítica de ConardLi relaja la originalidad en productos de datos y documentación. |
| 10 | Dirección propuesta: dorado = «lo que estudias» y «validado».  Jakub: un color, un significado. | Jakub: dorado solo para validado.  El foco se marca con tinta, tamaño, anillo y el velo sobre el resto. | Si un vecino validado y el foco comparten dorado, el lector no distingue «esto estudio» de «esto está comprobado», y eso enseña algo falso.  Si se mantiene la dirección original, el foco necesita además el rótulo «Estás aquí». |
| 11 | Jakub: rótulos en mayúsculas ≤14 px a 0.05em.  Convención cartográfica: versalitas muy espaciadas para áreas. | Cartografía en el lienzo, Jakub en HTML. | En un mapa el espaciado comunica extensión. |
| 12 | Receta Tufte: sin movimiento.  Emil: animación explicativa permitida. | Emil, solo para el trazado de una ruta nueva y nunca bajo movimiento reducido. | El orden del trazado explica la secuencia de pasos. |
| 13 | Elaya: esqueletos, no spinners.  Emil: un spinner rápido acorta la espera percibida. | Esqueleto para la ficha; texto de progreso para el mapa. | La forma de la ficha se conoce; la del mapa no. |
| 14 | book-serif-index y light-mode-paper-technical: textura de papel y tramas.  no-ai-design-slop y Tufte: cada marca tiene un trabajo. | Sin textura en superficies; tramado solo en «desconocida». | Una trama decorativa en todo el papel borraría la única trama con significado. |

---

## 3. Lista de verificación para cerrar la ronda

Compuertas binarias (iterate-until-verified).  Cada una se comprueba en captura real a 1440×900 y 390×844, claro y oscuro, salvo que diga otra cosa.  Quien revisa no es quien implementó.  Un hallazgo se anota como Antes / Después / Por qué (emil-design-eng) y con el criterio WCAG cuando aplica (accessibility-audit).

**A. Lo que puede enseñar algo falso**

- [ ] En modo principiante no aparece «Fuerza», ninguna escala numérica sin explicación, ningún identificador de Mathlib o Wikidata, ni «docstring», «testigo», «R7» o «score».
- [ ] Ruta derivada → integración: el encabezado dice «Conexión candidata; pendiente de revisión» o «Todavía no hay una conexión validada…», nunca ambos mensajes contradictorios en la misma tarjeta (R4-02).
- [ ] Ninguna conexión candidata usa trazo continuo dorado; ninguna validada usa punteado.
- [ ] El dorado aparece solo en elementos validados (contar en capturas de mapa, ficha y ruta).
- [ ] Ningún botón que lleve a código se llama «Ver concepto» o «Bajar a declaraciones»; dice «Ver respaldo técnico (código matemático)» (R4-06, R4-07).
- [ ] Ninguna región se rotula «aislada»; la región sin datos tiene tramado y el rótulo «desconocida».
- [ ] La ficha de número primo dice «divisores positivos» (R4-01).

**B. Orientación y lectura**

- [ ] Entrada: una pregunta, un buscador y tres recorridos; una sola acción rellena de tinta.
- [ ] Ficha: las cinco secciones (qué es, por qué importa, qué saber antes, con qué se conecta, cómo compruebo que lo entendí) en ese orden; las vacías dicen que aún no hay contenido revisado.
- [ ] Con ficha o ruta abiertas, «Volver al panorama» está visible sin scroll.
- [ ] Ruta: paradas numeradas con el mismo número en mapa, lista e índice textual; cada salto tiene una frase en lenguaje llano.
- [ ] Foco + vecinos a plena tinta; el resto atenuado y las etiquetas de área atenuadas siguen ≥3:1.
- [ ] Lista recortada muestra «Estas son las primeras 200 conexiones» y «Cargar más conexiones» (R4-05).

**C. Accesibilidad y teclado**

- [ ] Solo con teclado: Tab hasta el lienzo, `T` (o «Ver índice de conceptos»), el foco queda en el encabezado del índice y el siguiente Tab llega al primer concepto (R4-04).
- [ ] En móvil con ruta abierta, «Ver índice de conceptos» deja el índice como primer bloque visible y enfocado (R4-03).
- [ ] Anillo de foco visible en cada control de la cabecera, la ficha y la ruta, en claro y oscuro.
- [ ] Región `role="status"` anuncia «Buscando…», «12 resultados» y «Ruta lista: N pasos» (lector de pantalla o árbol de accesibilidad).
- [ ] A 320 px de ancho y con texto al 200 %, ficha y vista textual sin scroll horizontal ni recortes.
- [ ] Objetivos táctiles de 44 px en móvil.

**D. Tipografía y escritura**

- [ ] Ningún texto HTML por debajo de 12 px; captions a 13 px; cuerpo de ficha a 16 px con medida ≤62ch.
- [ ] Áreas en versalitas espaciadas, conceptos en serif regular, declaraciones solo en modo técnico.
- [ ] Contadores y números de parada con `tabular-nums`.
- [ ] Comillas «» en ES, “” en EN, «…» de un carácter; ningún «DESCRIPTION» ni texto en inglés en la interfaz española (H09).
- [ ] Botones con verbo y caso de oración; ningún «Ups», «!» ni «nosotros» en mensajes.
- [ ] Mismas pantallas en EN: nada se desborda; el nombre accesible del buscador cambia de idioma.

**E. Color y tema**

- [ ] Los valores de color en capturas coinciden con la tabla 2.3 (muestreo con cuentagotas en papel, ficha, tinta, dorado y candidato).
- [ ] No queda azul `#1f56b8` ni sepia `#7a5a2a` en la interfaz.
- [ ] Ningún texto sobre `--color-accent-solid`.
- [ ] El cambio de tema no deja transiciones a medio camino.

**F. Movimiento**

- [ ] Flechas, `+`/`−` y `N` mueven el mapa sin animación.
- [ ] La ficha entra en 200 ms y sale en 150 ms; ninguna animación de UI pasa de 300 ms salvo el trazado de ruta (600 ms).
- [ ] Con movimiento reducido activo: sin viaje de cámara, ruta completa al instante, ficha con fundido de opacidad.
- [ ] Arrastrar durante un viaje de cámara cancela el viaje sin salto.
- [ ] Con la vista textual activa, el bucle de render del mapa está detenido.

**G. Estados**

- [ ] Captura de cada estado de la tabla 2.6, en ES y EN.
- [ ] Error de red simulado: mensaje en línea y «Reintentar» funcional.

**H. Registro**

- [ ] `docs/STATUS.md` anota qué compuertas pasaron, cuáles quedaron bloqueadas y por qué, y la justificación de cualquier dependencia nueva.
- [ ] Capturas de antes y después con el mismo nombre en `docs/design/r4/`.

---

## 4. Alertas de seguridad

Ninguna de estas instrucciones se siguió.  Ningún script se ejecutó.  No se encontró texto del tipo «ignora tus instrucciones» dirigido al agente; las menciones que aparecen son reglas que piden tratar el contenido como datos.

| Ruta (relativa a `ext-skills/`) | Qué pide o trae | Cita corta |
|---|---|---|
| `codeswithroh_tastemaker/skills/tastemaker/SKILL.md` (pasos 0, 3 y 5) | Escribir memoria fuera del repo, en el directorio personal. | «check `~/.tastemaker/profile.md` (outside the repo, in the user's home directory)» |
| `codeswithroh_tastemaker/skills/tastemaker/SKILL.md` (paso 3) | Ejecutar scripts que salen a la red por fotos e iconos. | «Run `scripts/fetch_photos.py "<search terms>" --out design/assets/photos`» |
| `codeswithroh_tastemaker/skills/tastemaker/scripts/` | 23 scripts Python, 17 en `scripts/` y 6 en `ideagram/scripts/` (`fetch_photos.py`, `fetch_icons.py`, `_netguard.py` y otros) con red hacia `api.openverse.org`, `api.iconify.design` y `pixabay.com` (con `PIXABAY_API_KEY`). | «This skill contacts these hosts and no others» (SECURITY.md) |
| `codeswithroh_tastemaker/skills/tastemaker/references/component-sourcing.md:76-77` | Instalar CLI de terceros que configuran el agente y servidores MCP con token. | «`npx @21st-dev/cli@latest init --client <cursor\|claude-code\|windsurf>`»; «`--github-api-key <token>`» |
| `codeswithroh_tastemaker/skills/tastemaker/references/` (tech-stack-guides, animation-guidelines) | Instalar dependencias. | «`npm install gsap`», «`npm install motion`», «`npx shadcn@latest init`» |
| `codeswithroh_tastemaker/skills/tastemaker/references/interface-quality-rules.md` | Descargar una versión remota para usarla como guía. | «Fetch the current version when it matters» (`raw.githubusercontent.com/...`) |
| `codeswithroh_tastemaker/skills/tastemaker/SKILL.md` (paso 1.25) | Usar un MCP externo como fuente por defecto. | «Check the tool list for the `inspo` MCP» |
| `codeswithroh_tastemaker/worker/admin.sh`, `README.md:95-96` | Script de shell en el repo; instalar copiando a la configuración del agente. | «`cp -r /tmp/tastemaker/skills/tastemaker ~/.claude/skills/tastemaker`» |
| `ConardLi_garden-skills/skills/web-design-engineer/SKILL.md:94` | Descargar video y extraer cuadros con herramientas externas. | «official launch-video frames (`yt-dlp` + `ffmpeg`)» |
| `ConardLi_garden-skills/skills/web-design-engineer/SKILL.md` (paso 0) | Buscar en la web antes de diseñar. | «verify the current facts from authoritative sources» |
| `ConardLi_garden-skills/skills/web-design-engineer/SKILL.md:384, 430` | Cargar scripts de CDN no fijados por integridad. | «`https://unpkg.com/popmotion@11.0.5/...`», «`https://d3js.org/d3.v7.min.js`» |
| `ConardLi_garden-skills/README.md:363-442`, `scripts/release/*.mjs` | Instalar skills y descargar zips. | «`npx skills add ConardLi/garden-skills`», «`curl -fsSL -o "${SKILL}.zip"`» |
| `ConardLi_garden-skills/skills/web-design-engineer/agents/openai.yaml` | Activación implícita en Codex. | «`allow_implicit_invocation: true`» |
| `ConardLi_garden-skills/skills/web-design-engineer/README.md:15`, `dist/prompts/claude-design-system-prompt.md` | Procedencia: dice derivar del prompt de sistema de otro producto e incluir una copia. | «distills the core design philosophy from Claude Design's system prompt» |
| `MengTo_Skills/agent-skills/codex/optimize-web-animations/SKILL.md` (paso 6) | Borrar archivos temporales y hacer commit con un índice alterno. | «`rm -f /tmp/<task>-index`», «`GIT_INDEX_FILE=/tmp/<task>-index git commit`» |
| `MengTo_Skills/agent-skills/codex/optimize-web-animations/SKILL.md` (núcleo) | Restringir el navegador a usar. | «Do not use Chrome unless the user explicitly asks for it» |
| `MengTo_Skills/CLAUDE.md` | Instrucciones de repo ajeno: commit y cambio a una ruta de otra máquina. | «commit with a clear message»; «`cd /Users/mengto/clawd/@MengTo/Skills`» |
| `MengTo_Skills/scripts/*.mjs` | Scripts Node en el repo (sincronización y validación de demos). | `sync-neuform-skill-demos.mjs` |
| `emilkowalski_skills/skills/improve-animations/SKILL.md` | Modo que despacha un ejecutor que modifica código en un worktree; escribe en `plans/`. | «`execute <plan>` — Dispatch an executor subagent to implement the plan» |
| `emilkowalski_skills/skills/break-ui/SKILL.md` (fase 3) | Insertar un conmutador de datos dentro del producto. | «Put a segmented control labeled **Demo data / Worst case**» |
| `jakubkrehel_skills/skills/state-machine/SKILL.md` | Crear una ruta desechable dentro de la app. | «A scratch route inside the app holds the real component» |
| `jakubkrehel_skills/skills/technical-drawing/scripts/*.cjs`, `assets/*.js` | Scripts Node de prueba y construcción. | «Run the helper checks with `node scripts/test-geometry.cjs`» |
| `jakubkrehel_skills/skills/explain-interface/SKILL.md:40` | Agregar un servidor MCP de navegador. | «The `chrome-devtools-mcp` server is the easiest browser to add» |
| `Owl-Listener_designer-skills/design-research/skills/user-persona/SKILL.md:12` | Búsqueda web y guardar archivos en el espacio de trabajo. | «use web search to understand the product» |
| `elayadesign_ai-design-skills/skills/landing-page-design/SKILL.md` (alcance) | Remite a instalar otra skill de un repo externo. | «use `redesign-existing-projects` ([github.com/elayadesign/redesign-skill])» |

Nota sobre los demás archivos de los repos: hay carpetas fuera del alcance pedido (por ejemplo `ConardLi_garden-skills/skills/gpt-image-2/scripts/`, `web-video-presentation`, `codeswithroh_tastemaker/web/` y `worker/`) con código ejecutable y llamadas a APIs.  No se leyeron a fondo ni se ejecutaron.
