# Atlas · auditoría de usuario común

19 de octubre de 2026 · http://localhost:8765 · revisión externa, sin cambios al producto.

## 1. ¿Se entiende el sitio?

**A medias.** La primera pantalla permite decir «es un mapa para buscar ideas matemáticas y ver conexiones», por el título, el buscador y el botón Ruta. No permite saber qué se aprende ni cómo interpretar una conexión. Encontrar una idea conocida es posible; entenderla exige conocimientos que el visitante supuesto no tiene. La ficha suele ofrecer una descripción de catálogo, y los botones que prometen profundizar llevan a declaraciones de código. La ruta derivada → integración añade un problema nuevo: presenta simultáneamente «Sin ruta autorizada» y «Fuerza 5/5». No queda claro qué debe creer el lector.

## 2. Las cinco barreras principales

1. **No sé qué conexión puedo dar por válida.** La ruta conceptual de derivada a integración muestra fuerza máxima pese a depender de asignaciones candidatas y otras premisas; R4-02.
2. **Encuentro el nombre, pero no una explicación para aprender.** Persiste H04 de la auditoría pedagógica del 17: «operación en cálculo» no explica integración; [captura](r4/ficha-integral.png).
3. **Los botones para profundizar me llevan a código.** «Ver concepto» y «Bajar a declaraciones» no anticipan el cambio a una consulta técnica; R4-06 y R4-07.
4. **La alternativa textual no me lleva al texto.** En móvil queda debajo de la ruta; con teclado el foco continúa en el mapa; R4-03 y R4-04.
5. **La entrada no propone un primer objetivo.** Persiste H02 de la auditoría pedagógica: mapa denso, contador y controles, sin una pregunta inicial ni ejemplo de recorrido; [captura](r4/00-inicio.png).

## 3. Hallazgos nuevos, reproducidos

Cada fila se reprodujo dos veces. Las propuestas describen cambios de interfaz o contenido; no se implementaron. A = riesgo de enseñar algo falso; B = funcionamiento o consistencia; C = orientación; D = presentación visual. Los problemas anteriores se referencian después, sin abrirlos otra vez como hallazgos nuevos.

| id | prioridad | severidad | pantalla | pasos para reproducir | qué esperabas | qué pasó | captura | propuesta concreta de arreglo |
|---|---|---|---|---|---|---|---|---|
| R4-01 | A | confunde | Ficha «número primo», `wd:Q49008` | Buscar «número primo» → Enter → leer Intuición. Repetido en escritorio y móvil. | Una definición correcta de los divisores que se cuentan. | Dice «número natural mayor que 1 y que solo tiene dos divisores enteros». Falta «positivos»: 2 tiene los divisores enteros −2, −1, 1 y 2. La fórmula no explica esa restricción. No es un problema de relación entre nodos, sino de definición. | [Escritorio](r4/ficha-número-primo.png), [móvil](r4/movil-ficha-número-primo.png) | «Un número primo es un número natural mayor que 1 cuyos únicos divisores positivos son 1 y él mismo. Por ejemplo, 7 solo se divide exactamente entre 1 y 7». Explicar que τ cuenta divisores positivos o retirar esa notación del nivel inicial. |
| R4-02 | A | confunde | Ruta **derivada → integración**; salto «depende lógicamente de →», «recorrida al revés» | Ruta → Desde: derivada → Hasta: integral, seleccionar integración → Buscar ruta, con Mejor respaldada. | Una conclusión coherente y una distinción inequívoca entre conexión revisada y candidata. | Arriba dice «Sin ruta autorizada»; debajo muestra «Ruta por conceptos», «se usa para establecer» y **Fuerza 5/5**. En la misma tarjeta reconoce una asignación heurística no calibrada y otras premisas necesarias. Al desplegar, el testigo es `HasFDerivAt → hasFDerivAt_integral_of_dominated_of_fderiv_le''`, con varias premisas marcadas ✗. El 5/5 puede leerse como certeza máxima; no se explica allí qué mide. No afirmo que esa referencia formal sea falsa: el defecto es la conclusión que su presentación invita a sacar. | [Resultado](r4/ruta-derivada.png), [testigo y premisas](r4/bajar-derivada.png) | Encabezar «Conexión candidata entre estos conceptos; pendiente de revisión». Separar la falta de ruta validada del camino documental. Quitar la escala de fuerza del resumen para principiantes; en detalles, explicar exactamente qué mide. Escribir el salto desde el sentido recorrido: «Algunos resultados sobre integración utilizan resultados sobre derivadas; esto no basta para demostrarlos». |
| R4-03 | B | confunde | Vista textual móvil con ruta abierta, 390×844 | Obtener derivada → integración → pulsar Vista textual, sin cerrar la ruta. | Ver el índice textual o que el foco y el desplazamiento me lleven a él. | El botón pasa a «Mapa», pero sigo viendo mapa y formulario de ruta. El índice existe **después** de todo el panel de ruta: comienza aproximadamente en y=2240 respecto del viewport, en ambas comprobaciones. No está superpuesto ni falta del DOM; queda lejos y sin aviso. Desde inicio sin panel sí aparece debajo del mapa. | [Después de activar texto](r4/03-texto-tapado-por-ruta.png) | Al activar «Vista textual», llevar el índice al primer bloque visible y enfocarlo. Plegar mapa y ruta con controles explícitos «Mostrar mapa» y «Volver a mi ruta». Conservar los extremos de la ruta. |
| R4-04 | B | molesta | Navegación de teclado, mapa → índice | Llegar al lienzo con Tab → T → Tab. Repetir desde una sesión nueva. | Que T lleve al encabezado del índice y Tab al primer concepto. | T muestra la vista textual, pero conserva el foco en el canvas; Tab lleva a **Acercar (+)**. El visitante que acaba de pedir una alternativa textual continúa operando el mapa. No es una trampa permanente: se puede volver a recorrer la página con Tab. | [Foco en + con índice abierto](r4/05-foco-fuera-de-texto.png) | Enfocar «Vista textual» al abrirla; el siguiente Tab debe llegar al primer enlace del índice. Al regresar al mapa, restaurar su foco. |
| R4-05 | B | bloquea | Final de la ficha «número primo» | Buscar número primo → abrir ficha → bajar hasta «Se muestran 200 afirmaciones; hay más». | Un control para consultar las relaciones restantes de **esa ficha**. | Advierte el recorte pero no ofrece «Cargar más» ni paginación. Las conexiones visibles se concentran en «pertenece al área». La ruta número primo → criptografía sí utiliza «aparece en → Wikipedia (es): Número primo», que no está en esta porción de la ficha. El visitante no puede reconciliar ambas vistas ni acceder al resto desde aquí. «Ver concepto» abre otra lista, de declaraciones asignadas, no la continuación de estas relaciones. El bloqueo se limita a consultar el resto de la ficha. | [Límite sin control](r4/06-afirmaciones-sin-continuacion.png), [relación usada por la ruta](r4/ruta-número-primo.png) | Añadir «Cargar más conexiones» y «Filtrar por tipo de conexión». Mantener los grupos y mostrar «Mostrando 200; quedan conexiones sin cargar», sin aparentar que los conteos parciales son totales. |
| R4-06 | C | confunde | Ruta derivada → integración, «Bajar a declaraciones» | Obtener la ruta de R4-02 → pulsar Bajar a declaraciones. | Una explicación más concreta del salto, con nombres comprensibles. | Abre `HasFDerivAt` y `hasFDerivAt_integral_of_dominated_of_fderiv_le''`; añade «testigo», «R7», supuestos en inglés y premisas con ✗/✓. El mapa también pasa a esos identificadores. No explica qué significa la conexión ni qué significan las cruces para mi recorrido. El control funciona, pero para este visitante aumenta la confusión. | [Escritorio](r4/bajar-derivada.png), [móvil](r4/movil-declaraciones.png) | Renombrar «Ver respaldo técnico (código matemático)». Antes del código, explicar qué resultado usa cuál y aclarar «Las cruces indican premisas no aportadas por este recorrido; no significan que sean falsas». Ofrecer aparte «Entender esta conexión», solo cuando exista una explicación revisada. |
| R4-07 | C | confunde | Ficha integración → Ver concepto | Buscar integral → abrir integración → pulsar Ver concepto. | Una definición más desarrollada de la idea que ya estoy consultando. | Abre otra ficha con el mismo nombre y «operación en cálculo», seguida de declaraciones, asignaciones principales/secundarias, proyección conceptual y «nombre o docstring». La salida se llama «Ficha completa», aunque de allí venía. «Concepto» significa aquí una agrupación técnica que el rótulo no anticipa. | [Destino de Ver concepto](r4/concepto-integracion.png) | Cambiar «Ver concepto» por «Ver resultados técnicos agrupados» y «Ficha completa» por «Volver a la explicación de integración». Añadir «Esta lista reúne código relacionado; las asignaciones aún no están revisadas». No usar este botón como acción principal de aprendizaje. |

### Problemas anteriores que persisten

- **H03 / mejoras B02–B07:** Bayes sigue poniendo Thomas Bayes primero y mezclando código sin contexto; [captura](r4/buscar-Bayes.png). La prioridad de integral sí cambió: integración aparece primera.
- **H05:** derivada conserva «ÁREA» y relaciones agrupadas como «pertenece al área»; [captura](r4/ficha-derivada.png).
- **H06:** número primo → criptografía y Pitágoras → trigonometría terminan en conexiones documentales sin continuación de aprendizaje; [primos](r4/ruta-número-primo.png), [Pitágoras](r4/ruta-teorema-de-Pitágoras.png).
- **H09 / mejoras H06:** persisten instrucciones y metadatos en inglés dentro de la interfaz española; «DESCRIPTION» sigue en resultados; [captura](r4/buscar-Bayes.png).
- **Mejoras D07, pese a la corrección general descrita en CRITIQUE §8:** hay desbordamiento interno en móvil: panel de Pitágoras 390 px de ancho y 696 px de contenido; al bajar a declaraciones, 390/429 px; [recorte visible](r4/movil-declaraciones.png).
- **CRITIQUE, pendientes:** el mapa inicial móvil continúa pequeño y centrado, con mucho espacio vacío y etiquetas apretadas; [captura](r4/movil-inicio.png).
- **CRITIQUE §3–4 / mejoras D04–D06:** persiste la competencia entre etiquetas y aristas, también en oscuro; [panorama oscuro estabilizado](r4/oscuro-estable-1440.png).

Referencias: [auditoría pedagógica del 17](auditoria-pedagogica-2026-10-17.md), [64 mejoras](MPM-mejoras-funcionales-esteticas-pedagogicas.md), [CRITIQUE](../CRITIQUE.md).

## 4. Recorrido ideal de cinco pasos frente al actual

| Paso | Como debería verlo el visitante | Lo que ocurre hoy |
|---|---|---|
| 1. Entender la propuesta | «Explora cómo se conectan las ideas matemáticas. Busca una idea o empieza con un ejemplo». Accesos «Derivada e integral» y «Primos y criptografía». | Título, mapa y controles; no se desarrolla la promesa ni se ofrece un primer recorrido. |
| 2. Encontrar la idea | «Integral — acumulación y área con signo», con una breve descripción; resultados técnicos en otro grupo. | Integración ya sale primera, pero los resultados siguientes se mezclan con identificadores y la mayoría no explica su significado. |
| 3. Comprenderla | Una idea central, para qué sirve, un ejemplo resuelto y conocimientos previos con enlaces. | «Operación en cálculo» y conexiones de catálogo. No puedo responder por qué importa ni qué estudiar antes usando esa ficha. |
| 4. Entender la conexión | «La derivada mide cambios; la integral acumula. El teorema fundamental del cálculo las relaciona bajo ciertas condiciones», desarrollado en una explicación revisada; cada salto dice qué aporta. | Ruta conceptual de fuerza 5/5, aviso de falta de autorización y camino bibliográfico simultáneos. Bajar al detalle conduce a código. Esta propuesta editorial necesita revisión matemática antes de publicarse. |
| 5. Continuar y orientarse | Un siguiente concepto y «Volver al panorama»; vista textual visible y enfocada; idioma consistente. | Hay controles de zoom y regreso, pero la alternativa textual exige recuperar foco o desplazarse después de la ruta; cambiar idioma no convierte los detalles técnicos en explicación. |

## 5. Textos que reescribiría

Los rótulos ya señalados en auditorías anteriores aparecen aquí como propuestas de redacción solicitadas, no como hallazgos nuevos.

| Texto actual | Texto propuesto en español claro |
|---|---|
| «número natural mayor que 1 y que solo tiene dos divisores enteros» | «Número natural mayor que 1 cuyos únicos divisores positivos son 1 y él mismo». |
| «INTUICIÓN: operación en cálculo» | Si no hay explicación editorial: «Descripción breve: operación del cálculo». Cuando se escriba una explicación revisada: «Integrar permite calcular una acumulación; por ejemplo, el área con signo bajo una curva». |
| «Agrupa 876 declaraciones como concepto principal. Ver concepto» | «Hay resultados de código asociados a integración. Ver resultados técnicos agrupados». Dejar el conteo para el detalle. |
| «Bajar a declaraciones» | «Ver respaldo técnico (código matemático)». |
| «Ficha completa» en la agrupación conceptual | «Volver a la explicación de integración». |
| «Sin ruta autorizada» | «Todavía no encontramos una conexión validada con estos datos». Añadir «Esto no significa que las ideas no estén relacionadas». |
| «Fuerza 5/5» en la ruta conceptual observada | «Conexión candidata; pendiente de revisión». Explicar la métrica técnica aparte, sin presentarla como certeza. |
| «depende lógicamente de → / recorrida al revés» en el salto observado | «Algunos resultados sobre integración utilizan resultados sobre derivadas». Añadir que también necesitan otras premisas. |
| «La afirmación testigo, como ruta entre declaraciones» | «Este es uno de los resultados de código que respalda la conexión propuesta». |
| «Se muestran 200 afirmaciones; hay más» | «Estas son las primeras 200 conexiones». Botón: «Cargar más conexiones». |
| «nombre o docstring» | «Coincidencia con el nombre o la descripción del código». |
| «DESCRIPTION» | «Descripción». |
| «Vista textual» / «Mapa» como alternancia que mantiene ambos | «Ver índice de conceptos» / «Volver al mapa», haciendo que cada acción lleve al contenido anunciado. |

## Registro de tareas, alcance y mediciones

### Búsquedas y lectura

Se ensayaron los términos siguientes dos veces en cada tamaño, con la interfaz española. Integral, Bayes y pi tienen la misma grafía en ambos idiomas; no se cuentan como traducciones distintas. La captura y el texto de cada búsqueda están en `r4/busquedas.json` y `r4/movil.json`.

| Español / inglés buscado | Primer resultado observado | ¿Responde a la intención inicial? |
|---|---|---|
| integral / integral | integración | Sí para localizarla; no explica su significado en el resultado. |
| derivada / derivative | derivada | Sí, aunque «ÁREA» resulta extraño. |
| teorema de Pitágoras / Pythagorean theorem | teorema de Pitágoras | Sí; al abrirlo, «principio matemático» no explica el triángulo ni sus lados. |
| Bayes / Bayes | Thomas Bayes | Ambiguo; para quien busca el teorema, obliga a elegir el segundo resultado. |
| número primo / prime number | número primo | Sí; la definición abierta contiene R4-01. |
| pi / pi | pi | Sí; «constante matemática» no explica por sí sola la razón entre circunferencia y diámetro. |
| matriz / matrix | matriz | Sí; describe filas y columnas, pero no desarrolla utilidad ni ejemplo. |

Además se abrió explícitamente teorema de Bayes. En las siete fichas puedo reconocer el nombre; en algunas, leer una definición mínima o fórmula. No obtengo de forma consistente por qué importa, qué debo saber antes y una conexión explicada. No se confirmó un homónimo nuevo equivalente al antiguo integración/IsIntegral: ese enlace ya no apareció en la ficha de integración inspeccionada. Las candidatas ahora están separadas y la puntuación se llama «heurística, no calibrada». No se repite el antiguo hallazgo como si siguiera igual.

### Rutas, mapa, teclado e idioma

- Se repitieron en ambos tamaños número primo → criptografía, derivada → integración y teorema de Pitágoras → trigonometría. Los caminos documentales aclaran que no implican dependencia ni acuerdo; no se observó que una cita se presentara directamente como demostración. La dificultad específica nueva es R4-02.
- Acercar, alejar y Vista completa respondieron; también flechas, +/−, N y Enter. No se observó una región rotulada «aislada» por carecer de datos en estos recorridos. Esto no certifica todas las regiones.
- El mapa mezcla conceptos con nombres como `Mathlib`, `sublime calculation` y `everyday mathematics`. Por su nombre no puede deducirse que sean errores matemáticos; para el visitante no funcionan como orientación. No se imputó una asociación falsa sin abrir y comprobar sus extremos.
- Se completó la ruta derivada → integración y su despliegue **solo con Tab, escritura y Enter**, dos veces, además de los atajos del mapa. No se usó lector de pantalla. El desvío de foco de R4-04 quedó registrado con el elemento activo y captura.
- Se cambió español → inglés y se volvió a español en escritorio; también se revisó inglés en móvil. Los títulos y gran parte de las fichas se traducen. En inglés, el nombre accesible del buscador y su anuncio conservan «Buscar» / «12 resultados» tras el cambio. En español siguen rótulos de resultados como DESCRIPTION y textos técnicos en inglés. Los alias que identifican expresamente su idioma no se consideran errores de traducción. Se esperó a que la vista terminara de actualizarse antes de juzgar las etiquetas.
- Las URLs observadas usan `e=`, `c=`, `text=1` y `path=1&from=…&to=…`; no se encontró el parámetro exacto `p=` en las navegaciones ensayadas. No se infiere un defecto por la mera presencia de `path=`.

### Cómo se comprobó

Edge real sin interfaz, controlado con Playwright; viewports exactos **1440×900** y **390×844**, sin emulación de red móvil. El servidor ya respondía y no se inició otra instancia. Se exploró la interfaz antes de leer las críticas. Antes de redactar se leyeron CRITIQUE y las dos auditorías previas existentes en la carpeta. No se leyó ni cambió código del producto.

Se conservaron dos pasadas en los JSON de evidencia. Las capturas corresponden principalmente a la segunda pasada. Las cifras de nodos, declaraciones y afirmaciones citadas en el informe son **rótulos de la interfaz**, no conteos independientes de la base. Las dimensiones de desbordamiento proceden de `clientWidth` y `scrollWidth`; la posición del índice móvil, de `getBoundingClientRect`. El documento completo mantuvo 390 px: el recorte señalado es interno al panel, no un ensanchamiento global de la página.

En los registros de consola no hubo `pageerror`, advertencias ni errores de consola en las pasadas que los capturaron; las respuestas registradas no incluyeron HTTP ≥400. Los fallos de interfaz descritos no dependieron de errores de red. Evidencia: `red-consola-1.json`, `red-consola-2.json`, `red-consola-movil.json`, `red-consola-teclado.json` y `final-checks.json`.

Mediciones locales, sin convertirlas en una promesa de rendimiento:

| Operación y método | Pasada 1 | Pasada 2 |
|---|---:|---:|
| Inicio escritorio: reloj antes de `goto` hasta que el contador contiene «nodos» | 390 ms | 128 ms |
| Buscar integral en escritorio: desde terminar de llenar el campo hasta que aparece la primera opción | 194 ms | 212 ms |
| Buscar integral en móvil: mismo criterio | 206 ms | 204 ms |
| Ruta derivada → integración: desde Enter en Buscar ruta hasta que aparece Bajar a declaraciones | 44 ms | 46 ms |

En las cuatro pasadas instrumentadas de inicio, tema y búsqueda, la petición más lenta registrada duró **30,684 ms**, según `request.timing().responseEnd` de Playwright. No hubo peticiones superiores a 2 s en esa muestra. Esto mide solicitudes de red, **no** el tiempo hasta que cada etiqueta del mapa termina de dibujarse. El indicador de nodos tampoco certifica que el render haya finalizado. No se detectó una carga mayor de 2 s con esos criterios; no se hizo una prueba de caché fría, equipos lentos ni red remota.

Se revisaron capturas en claro y oscuro estabilizado. No se hizo una medición completa de contraste WCAG, ni una auditoría exhaustiva del corpus. La sospecha inicial «el botón Vista textual no funciona en móvil» quedó **no reproducida como inactividad**: el índice sí se crea, pero queda después de la ruta; el hallazgo acotado y reproducido es R4-03. No se reportan problemas intermitentes sin esa distinción.

Evidencia principal adicional: [recorridos](r4/recorridos.json), [móvil](r4/movil.json), [teclado y mapa](r4/teclado-mapa.json), [comprobaciones finales](r4/final-checks.json), [tiempos de red](r4/tiempos-red.json), [límite de ficha](r4/limite-ficha.json).
