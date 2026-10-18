# MPM: 64 mejoras funcionales, estéticas y pedagógicas

Auditoría ampliada del despliegue http://127.0.0.1:8765/ · 17 de octubre de 2026.

## Dictamen

En las pantallas examinadas, MPM presenta con más madurez su corpus y su procedencia que una experiencia de aprendizaje. Hay una base útil para investigar conexiones, pero el principiante todavía debe decidir qué estudiar, interpretar el grafo, distinguir relaciones heterogéneas y construir por sí mismo una explicación.

El problema estético principal es la jerarquía. La pantalla entrega mucho espacio y atención a un grafo denso, mientras la explicación queda confinada a una ficha estrecha. El problema funcional es la falta de orientación entre acciones. El problema pedagógico es que muchas fichas son registros documentales presentados con nombres de secciones educativas. El problema más urgente es semántico: al menos una correspondencia equivocada aparece como candidata a formalización con una puntuación que parece certeza.

La aspiración de diseño es que el usuario pueda responder, en cualquier momento: qué estoy estudiando, por qué importa, qué debo entender antes, qué significa esta conexión y cómo puedo comprobar si lo comprendí.

No hay razón para deducir, a partir de esta revisión, que se deba cambiar Rust o reemplazar el sistema completo. La intervención propuesta afecta principalmente al modelo editorial del contenido, la selección de lo visible, la semántica de las relaciones y el diseño de interacción.

## Evidencia y límites

Se combinaron dos recorridos reales del navegador, con capturas en los tamaños disponibles de aproximadamente 757 × 764 y 1280 × 720 píxeles. No se cambiaron los tamaños para simular dispositivos. Se inspeccionaron contenido accesible, capturas y medidas DOM de la ficha de derivada. No se realizaron una auditoría de código, mediciones de rendimiento, una certificación WCAG ni pruebas con alumnos. No se probaron el tema claro ni todos los idiomas y dispositivos.

E01. Entrada: el contador indica 318.733 nodos. El mapa mezcla nombres matemáticos, archivos o módulos, ISBN, fechas y otros documentos. Hay numerosas etiquetas próximas o superpuestas y tamaños cuya interpretación no aparece junto al mapa en las capturas iniciales.

E02. Búsqueda de `integral`: en el primer recorrido aparecen varias declaraciones de Mathlib antes de `integración`, en noveno lugar. Búsqueda de `Bayes`: mezcla persona, teorema y declaraciones. La pregunta «¿para qué sirve una derivada?» devuelve primero derivada paramétrica y sitúa derivada general después de varias variantes. Esto demuestra recuperación de términos, pero no una respuesta a la intención educativa de esa pregunta.

E03. La lista de sugerencias es estrecha en pantalla amplia. Algunas etiquetas se reparten en muchas líneas mientras la categoría ocupa una parte importante del ancho; aparecen rótulos en inglés como DESCRIPTION. No todas las sugerencias incluyen una descripción.

E04. Derivada aparece como ÁREA y su intuición dice «operación en cálculo infinitesimal». Integración dice «operación en cálculo». Bayes muestra una descripción general y una fórmula sin un ejemplo trabajado ni explicación de símbolos en la ficha inspeccionada.

E05. Integración enlaza a `IsIntegral` como propuesta de «formalizado como», con `p=1.00`. El destino trata de raíces de polinomios mónicos sobre anillos. El sitio declara que la puntuación es heurística y no calibrada. La asociación sigue presente al revisarla de nuevo en esta ampliación.

E06. En derivada, «pertenece al área» reúne conexiones procedentes de propiedades distintas, incluidas P279, P31, P2579 y P361. Cálculo diferencial aparece varias veces. Las flechas entrantes y salientes exigen interpretar la dirección de la oración.

E07. Derivada → integración devuelve «Sin ruta autorizada» tanto con Mejor respaldada como con Más intuitiva. El resultado explica el límite documental con texto como «fuerza 0 < 1» y muestra una cadena por Wikipedia, sin continuación pedagógica en esa pantalla.

E08. Cambiar derivada de Estructura a Aprendizaje conserva el contenido de su ficha. Aplicación en Bayes ofrece enlaces a estadística y probabilidad bayesianas, sin desarrollar un caso aplicado. Esto no demuestra que las capas no filtren el mapa: el hallazgo se refiere a la experiencia explicativa de la ficha.

E09. La vista textual enumera muchas regiones y entidades. Los totales observados cambian con la vista: en un recorrido hubo 75 regiones y 325 entidades; en otro, 121 regiones. Aparecen fuentes como ISBN, Estados Unidos y fechas junto a conceptos. Son conteos de vistas concretas, no cantidades fijas del producto.

E10. Comparar regiones pide seleccionar mediante Mayús+clic o desde el panel; no presenta dos buscadores en su estado inicial. Al comparar Set y Mathlib/LinearAlgebra/Matrix muestra miembros, ocultos a este zoom, centralidad, relaciones agregadas y «Aristas entre ambas en la vista: ninguna». No explica qué se está comparando matemáticamente. La lista muestra cantidades de ocultos junto al rótulo «Miembros», mientras el detalle diferencia miembros totales y ocultos; conviene revisar esa consistencia.

E11. El formulario de rutas ofrece objetivos como Más intuitiva y Menos conceptos nuevos. Las opciones avanzadas muestran Saltos máx., Incluir disputadas y un campo Pesos con ejemplo `evidence:1,length:0.2`.

E12. En la captura de 1280 píxeles, la ficha de derivada mide 376 píxeles de ancho de artículo. Su cuerpo introductorio usa 17 px y línea de 25.5 px; encabezados de sección y algunos metadatos, 11 px; botones, 12.5 px. Varias cadenas de procedencia tienen anchura desplazable superior a su contenedor (por ejemplo 387 frente a 362 píxeles). La captura muestra desplazamiento horizontal en el panel. No se midió contraste de color.

E13. Hay búsqueda accesible, foco visible en la búsqueda, fórmulas representadas como matemáticas en el DOM, controles de zoom con nombre, alternativa textual, instrucciones de teclado, botón Atrás en el recorrido ampliado y enlaces profundos. La auditoría no parte de que esas funciones no existan; propone hacerlas más útiles y comprobarlas de extremo a extremo.

## Cómo utilizar esta lista

Prioridad P0: evitar información engañosa antes de promover la herramienta para aprender. P1: cambios que desbloquean orientación, lectura y comprensión. P2: profundidad, comodidad y calidad del producto completo.

Base O: respuesta a una observación concreta; D: propuesta de diseño, sin afirmar que la capacidad esté ausente en todas las partes del sistema; V: comprobación pendiente. Las categorías describen el motivo principal, aunque muchos cambios tienen efectos funcionales, estéticos y pedagógicos a la vez.

## A. Entrada, orientación y navegación

1. **A01 · P1 · O/D · Entrada por objetivos.** Añadir «¿Qué quieres entender?» con preguntas concretas y acceso por áreas. E01–E04. La primera decisión debe ayudar a iniciar una actividad. **Aceptación:** un usuario nuevo puede empezar una explicación sin conocer un identificador o nombre técnico exacto.

2. **A02 · P1 · D · Intenciones de uso explícitas.** Diferenciar Aprender, Explorar e Investigar sin triplicar el producto ni encerrar a las personas en un nivel fijo. Cada intención reorganiza la misma base. **Aceptación:** se puede cambiar de intención conservando concepto y contexto.

3. **A03 · P1 · O/D · Ubicación comprensible.** Mostrar área, tema, concepto y, cuando corresponda, etapa del recorrido. E01, E08. **Aceptación:** el lector sabe dónde está y puede volver a su recorrido después de una digresión.

4. **A04 · P1 · O/D · Una acción principal por contexto.** En una lección, priorizar «Entender esta idea» o «Continuar»; en un recurso, «Leer la fuente». E04, E12. **Aceptación:** las acciones de mapa y ruta no desplazan al objetivo de lectura en la primera pantalla.

5. **A05 · P2 · V/D · Regreso con contexto.** Comprobar que Atrás restaure selección, posición, filtros y avance de lectura, y distinguir volver de cerrar. Existe un botón Atrás; E13. **Aceptación:** explorar una conexión y volver no obliga a reconstruir el punto de partida.

6. **A06 · P2 · D · Retomar y guardar.** Permitir marcadores, notas y recorridos recientes de forma voluntaria, con funcionamiento local si no se desea una cuenta. **Aceptación:** recuperar una idea y el motivo por el que se guardó resulta directo.

7. **A07 · P1 · O/D · Estados sin salida que orientan.** Diferenciar falta de cobertura, ausencia de resultados, error de carga y falta de ruta validada. E07. **Aceptación:** cada estado ofrece una acción útil y no sugiere inexistencia matemática a partir de datos incompletos.

8. **A08 · P1 · O/D · Explicar las vistas.** Acompañar cada capa con la pregunta que responde y los tipos de conexiones que muestra. E08. **Aceptación:** el usuario entiende qué cambió, qué permanece y cómo leer los elementos de la vista activa.

## B. Búsqueda y desambiguación

9. **B01 · P1 · O · Prioridad conceptual en Aprender.** Reordenar `integral` para que el concepto introductorio no quede detrás de declaraciones de una biblioteca. E02. **Aceptación:** una batería de consultas cotidianas encuentra su entrada educativa pertinente entre los primeros resultados.

10. **B02 · P1 · O/D · Resultados por tipo.** Agrupar conceptos, teoremas, formalizaciones, personas y fuentes, con grupos plegables. E02. **Aceptación:** no hace falta leer un namespace para distinguir una explicación de un artefacto formal.

11. **B03 · P0 · O/D · Desambiguación por significado.** Mostrar dominio y definición breve para homónimos como integral. E05. **Aceptación:** cálculo integral e integralidad algebraica no se presentan como variantes intercambiables de una misma idea.

12. **B04 · P1 · O/D · Alias e idioma del lector.** Normalizar sinónimos, acentos y variantes; usar el nombre original como apoyo cuando no hay traducción. E02–E04. **Aceptación:** una consulta en español no queda dominada por coincidencias de identificadores ingleses sin contexto.

13. **B05 · P2 · O/D · Reconocer preguntas educativas.** «¿Para qué sirve una derivada?» debe conducir al concepto y a su propósito, con evidencia para cualquier respuesta generada. E02. **Aceptación:** se distingue localizar palabras de responder a la intención; el sistema declara sus límites cuando no puede responder.

14. **B06 · P1 · O · Panel de resultados legible.** Dar a las sugerencias un ancho independiente del pequeño campo de la barra. E03. **Aceptación:** título, significado y tipo caben en una composición ordenada; las categorías largas no aplastan el título.

15. **B07 · P1 · O/D · Descripciones editoriales consistentes.** Reemplazar rótulos como DESCRIPTION por texto localizado y explicar qué encontrará el usuario al abrir el resultado. E03. **Aceptación:** cada resultado educativo presenta una frase útil; la ausencia de descripción no se disfraza de explicación.

16. **B08 · P2 · D/V · Búsqueda avanzada y recuperación.** Mantener un acceso explícito a identificadores y fórmulas, filtros por tipo y ampliación de resultados; probar errores ortográficos y consultas sin coincidencias. **Aceptación:** el experto conserva precisión y el principiante recibe alternativas pertinentes.

## C. Mapa y comparación

17. **C01 · P1 · O/D · Proyección pedagógica del corpus.** Seleccionar conceptos y conexiones educativas para Aprender; conservar módulos, infraestructura y documentación en otras vistas. E01, E09. **Aceptación:** la prominencia de ISBN o Tactic no define la geografía inicial de aprendizaje.

18. **C02 · P1 · O/D · Zoom con significado.** Mostrar áreas al alejarse, conceptos al acercarse y enunciados al profundizar, con transiciones comprensibles. E01. **Aceptación:** aumentar el detalle no equivale simplemente a añadir etiquetas sobre las existentes.

19. **C03 · P1 · O · Control de colisiones.** Definir prioridad y presupuesto de etiquetas, gestionar colisiones y ofrecer detalles al seleccionar. E01. **Aceptación:** ninguna etiqueta imprescindible de la selección queda ilegible por superposición en las vistas de referencia.

20. **C04 · P1 · O/D · Leyenda y semántica visual.** Explicar tamaño, color, distancia, borde, flechas y agregación. E01. **Aceptación:** el lector sabe qué magnitudes se representan y que una proximidad de layout no prueba una relación matemática.

21. **C05 · P1 · O/D · Vecindario del concepto activo.** Enfocar conocimientos previos, relaciones explicativas y aplicaciones elegidas, con expansión voluntaria. E01, E08. **Aceptación:** seleccionar Bayes ofrece un contexto útil para Bayes y una forma clara de volver al panorama.

22. **C06 · P1 · O/D · Conexiones legibles al seleccionarlas.** Mostrar oración, dirección, contexto y razón de relevancia. E06. **Aceptación:** el alumno puede leer la conexión sin deducirla solo del color o la orientación de una línea.

23. **C07 · P1 · O/D · Comparación basada en preguntas.** Añadir selectores buscables y una comparación de objetos, problemas, métodos, diferencias y puentes. E10. **Aceptación:** «Comparar» explica algo sobre las regiones; los conteos y la centralidad quedan como detalle analítico con denominadores claros.

24. **C08 · P2 · O/D · Alcance visible de cada panorama.** Mostrar filtros y cobertura, distinguir total, visible y oculto, y mantener referencias espaciales estables. E09, E10. **Aceptación:** «ninguna arista en esta vista» no se interpreta como ausencia de relación entre campos.

## D. Lectura y dirección estética

25. **D01 · P1 · O/D · Modo de lectura protagonista.** Ampliar la lección y reducir el mapa a contexto plegable cuando se estudia. E12. **Aceptación:** el lector puede trabajar con una explicación larga sin quedar confinado al ancho de un inspector lateral.

26. **D02 · P1 · O/D · Composición según la tarea.** Mantener mapa amplio en Explorar y texto amplio en Aprender; adaptar la ficha a pantallas estrechas sin duplicar navegación. E12. **Aceptación:** las proporciones responden a lo que se está haciendo, no a una división fija de pantalla.

27. **D03 · P1 · O/D · Escala tipográfica clara.** Distinguir título, idea central, explicación, instrucciones, evidencia y metadatos. E12. Conservar una serif de lectura si funciona; no exigir un cambio de fuente por moda. **Aceptación:** la jerarquía es reconocible sin depender de muchos bordes o mayúsculas.

28. **D04 · P1 · O/D · Texto auxiliar suficientemente legible.** Revisar los rótulos de 11 px y la densidad de botones/metadatos. E12. **Aceptación:** instrucciones y estados importantes se leen cómodamente y soportan aumento de texto; el tamaño propuesto se valida con usuarios.

29. **D05 · P1 · O/D · Espacio y ritmo.** Separar bloques por función, limitar acumulaciones de chips y establecer una escala coherente de márgenes. E03, E12. **Aceptación:** se reconoce la idea principal, una actividad y su explicación como unidades distintas al recorrer la página.

30. **D06 · P1 · O/D · Colores con una función limitada.** Separar selección, tipo de objeto, estado de evidencia y avance mediante combinaciones de color, forma y texto. E01. **Aceptación:** no se exige memorizar un color para cada significado ni se confunde verificación con dominio del alumno.

31. **D07 · P1 · O · Resolver el desbordamiento.** Ajustar URLs e identificadores, permitir copia del original y aislar desplazamiento de fórmulas que realmente lo necesiten. E12. **Aceptación:** el texto explicativo no obliga a desplazamiento horizontal por una cadena de procedencia.

32. **D08 · P2 · D/V · Movimiento al servicio de la orientación.** Animar transiciones solo cuando ayuda a seguir un cambio de foco, respetando movimiento reducido. **Aceptación:** no hay movimiento decorativo que compita con lectura o práctica; medir estabilidad y comportamiento del mapa antes de ajustar.

## E. Significado, evidencia y honestidad editorial

33. **E01 · P0 · O · Corregir integración/IsIntegral.** Separar los dos conceptos y retirar esa propuesta de puentes didácticos establecidos. E05 de la evidencia. **Aceptación:** la relación equivocada se conserva, si hace falta, únicamente en historial/revisión, sin enseñar equivalencia.

34. **E02 · P0 · O · Evitar falsa precisión.** Sustituir `p=1.00` por una descripción honesta de puntuación heurística, o no mostrarla al estudiante. E05. **Aceptación:** ninguna cifra se presenta como probabilidad de verdad sin una estimación calibrada y un evento definido.

35. **E03 · P0 · O/D · Separar candidatas y establecidas.** No encabezar una propuesta incierta con el mismo lenguaje de una formalización confirmada. E05. **Aceptación:** la condición de candidata se entiende antes de abrir detalles y su elegibilidad para una ruta educativa es explícita.

36. **E04 · P0 · O · Corregir tipos y verbos.** Revisar derivada como ÁREA y la agrupación de relaciones bajo «pertenece al área». E04, E06. **Aceptación:** cada relación conserva una interpretación precisa, incluso al recorrerla al revés.

37. **E05 · P1 · O/D · Agregar evidencia sin duplicar ruido.** Agrupar destinos cuando la afirmación sea equivalente, manteniendo fuentes separadas; no fusionar significados distintos. E06. **Aceptación:** el lector ve una explicación clara y puede inspeccionar todas sus evidencias.

38. **E06 · P0 · O/D · Separar hipótesis de limitaciones técnicas.** Distinguir condiciones matemáticas, fidelidad de traducción, compilación, axiomas y estado de extracción. En IsIntegral se vieron limitaciones de importación bajo SUPUESTOS. **Aceptación:** «axiomas no calculados» nunca se lee como una hipótesis del concepto.

39. **E07 · P1 · O/D · Fuente pertinente a la afirmación.** Ofrecer el fragmento y localizador exactos y diferenciar origen del dato de respaldo de la explicación. E04–E06. **Aceptación:** una fuente verificable justifica lo que se afirma, con atribución conservada y detalles técnicos accesibles.

40. **E08 · P2 · D · Ciclo editorial visible.** Mostrar si una ficha es documental, explicada, revisada o pendiente; permitir señalar errores y registrar correcciones. **Aceptación:** cobertura de datos y cobertura pedagógica tienen indicadores distintos, con responsables/versiones cuando corresponda.

## F. Aprendizaje dentro de la ficha

41. **F01 · P1 · O/D · Objetivo observable.** Abrir con «Al terminar podrás…» referido a una acción concreta. E04. **Aceptación:** el objetivo puede comprobarse con una explicación, clasificación, construcción o problema.

42. **F02 · P1 · D · Conocimientos previos con ayuda.** Mostrar pocos prerrequisitos explicados y permitir una comprobación breve o un enlace de recuperación. **Aceptación:** el alumno puede continuar o reforzar una idea sin quedar atrapado en una cadena interminable de requisitos.

43. **F03 · P1 · O · Intuición que explique.** Sustituir la descripción de catálogo por una idea causal, visual o conceptual revisada. E04. **Aceptación:** la sección permite anticipar algo sobre el concepto; una mera clasificación permanece rotulada como descripción.

44. **F04 · P1 · O/D · Ejemplo trabajado.** Desarrollar un caso con pasos, decisiones y justificación. E04. **Aceptación:** el usuario comprende por qué se hace cada paso y puede contrastar su razonamiento con el del ejemplo.

45. **F05 · P1 · O/D · Símbolos y condiciones explicados.** Conectar cada símbolo con el ejemplo, con aclaraciones mediante clic, foco y texto, no solo hover. E04. **Aceptación:** en Bayes se entiende P(A|B), se distingue de P(B|A) y se indica P(B)>0 para la fórmula elemental mostrada.

46. **F06 · P1 · D · Errores frecuentes y límites.** Incluir un contraejemplo o una confusión habitual con explicación. **Aceptación:** el alumno sabe cuándo una regla no se aplica; no se convierte una intuición aproximada en una afirmación universal.

47. **F07 · P1 · D · Práctica y retroalimentación específica.** Pedir predicción, explicación o solución; ofrecer pistas graduales. **Aceptación:** una respuesta incorrecta recibe orientación sobre el razonamiento, no solo una cruz o la respuesta final.

48. **F08 · P2 · D · Transferencia y recuerdo.** Añadir un problema nuevo y una comprobación diferida voluntaria; registrar avance por evidencia de comprensión. **Aceptación:** abrir una ficha o completar clics no equivale a dominar el concepto.

## G. Recorridos, aplicaciones y descubrimiento

49. **G01 · P1 · O/D · Separar ruta del grafo y secuencia de estudio.** Conservar la primera para relaciones y evidencia; dar a la segunda objetivos y actividades. E07, E11. **Aceptación:** el usuario sabe cuál solicita y qué garantiza cada resultado.

50. **G02 · P1 · D · Configurar la ruta en términos del alumno.** Preguntar qué sabe, qué quiere poder hacer y cuánto detalle desea; permitir saltar preguntas. **Aceptación:** «Menos conceptos nuevos» se refiere a conocimientos declarados o comprobados, con incertidumbre explícita.

51. **G03 · P1 · O/D · Explicar cada transición.** Mostrar qué se gana al pasar al siguiente concepto y por qué es pertinente. E07. **Aceptación:** un recorrido se puede leer como una secuencia de razonamiento y no solo como nombres unidos.

52. **G04 · P1 · O/D · Promesas de ruta comprobables.** Explicar qué cambia entre Más intuitiva, Más formal y Mejor respaldada; traducir opciones avanzadas para aprendizaje y conservar pesos técnicos en herramientas expertas. E11. **Aceptación:** las etiquetas tienen un efecto comprensible y no prometen personalización sin información del alumno.

53. **G05 · P1 · O/D · Recuperación cuando falta un puente.** Mostrar la laguna y ofrecer material revisado o alternativas de estudio disponibles, sin relajar el significado de las relaciones. E07. **Aceptación:** el mensaje describe un límite de MPM y permite continuar sin inventar una demostración.

54. **G06 · P1 · O/D · Casos aplicados completos.** Desarrollar problema humano, variables, modelo, cálculo, interpretación y límites. E08. **Aceptación:** «se aplica en» permite comprender qué hace realmente la matemática en ese contexto.

55. **G07 · P2 · D · Laboratorios que enseñan.** Integrar controles con predicción previa, observación y explicación posterior; enlazar la simulación con la fórmula. **Aceptación:** cambiar un parámetro produce una observación interpretable y la experiencia tiene alternativa textual.

56. **G08 · P2 · D · Analogías con fronteras.** Mostrar qué estructura se conserva entre disciplinas, qué se pierde y qué partes son hipótesis. **Aceptación:** un puente sugerente no se presenta como equivalencia, causalidad o aplicación validada sin evidencia.

## H. Accesibilidad, estados y validación del producto

57. **H01 · P1 · O/D · Índice textual diseñado para aprender.** Organizar objetivo, conceptos previos, conexiones explicadas y próximos pasos antes de listas extensas. E09. **Aceptación:** es posible comprender el recorrido sin interpretar el lienzo y sin atravesar cientos de referencias.

58. **H02 · P1 · V · Teclado y foco de extremo a extremo.** Probar buscar, elegir, abrir, expandir, recorrer, comparar y volver. Hay soporte anunciado; E13. **Aceptación:** la secuencia completa se realiza sin ratón, sin perder el foco y con selección identificable.

59. **H03 · P1 · V/D · Lectores de pantalla y matemáticas.** Validar lectura de fórmulas, dirección de relaciones, cambios de panel y resultados dinámicos; proporcionar alternativas equivalentes. **Aceptación:** una persona puede realizar las mismas tareas esenciales sin depender de la representación visual.

60. **H04 · P1 · V · Reflujo, contraste y objetivos táctiles.** Evaluar ambos temas, ampliación de texto, pantallas estrechas, contraste y controles. **Aceptación:** los resultados se documentan contra WCAG 2.2 y tareas reales; las excepciones del mapa bidimensional no se aplican indiscriminadamente al texto.

61. **H05 · P2 · V/D · Estados de carga y error comprensibles.** Distinguir cálculo en curso, contenido vacío, fallo y actualización, manteniendo lo ya leído y ofreciendo reintento. No se midieron tiempos en esta auditoría. **Aceptación:** la interfaz nunca obliga a adivinar si falta información o si la operación sigue ejecutándose.

62. **H06 · P1 · O/V · Localización completa.** Traducir rótulos como DESCRIPTION y explicar textos originales sin modificar su sentido; diferenciar el idioma de la interfaz y el de la fuente. E03, E04. **Aceptación:** un lector hispanohablante puede completar una lección sin interpretar instrucciones técnicas inglesas.

63. **H07 · P1 · D · Pruebas con tareas de comprensión.** Comparar presentación actual y propuesta con estudiantes de distintos niveles: localizar, explicar símbolos, interpretar conexiones y resolver un caso nuevo. **Aceptación:** se registran errores de interpretación y comprensión, además de navegación; no se presenta tiempo en pantalla como aprendizaje.

64. **H08 · P2 · D/V · Biblioteca de estados y regresión.** Documentar diseños de búsqueda, ficha, ruta, fórmula larga, contenido incompleto, evidencia discutida y pantallas estrechas; mantener los casos semánticos observados como regresiones. **Aceptación:** una mejora del mapa no rompe lectura, accesibilidad o fidelidad de las relaciones.

## Experiencia objetivo

### Pantalla inicial

Una pregunta principal («¿Qué quieres entender?»), ejemplos de objetivos, entrada por áreas y opción visible de exploración libre. Una persona puede empezar sin configurar su perfil. El contador del corpus y los detalles de cobertura son información secundaria, accesible cuando se necesita.

### Lección

Un área central de lectura, objetivo e idea esencial al principio; ejemplo y actividad; formulación explicada; errores habituales; conexiones seleccionadas y siguiente paso. Un mapa contextual acompaña la lección y puede ampliarse. Fuentes, formalización y procedencia tienen acceso directo sin dominar la primera pantalla.

### Conexión

Una oración completa une los conceptos. El panel explica el papel de la relación, las condiciones, un ejemplo y la evidencia. El usuario puede distinguir relación matemática, ayuda pedagógica, antecedente histórico, aplicación y propuesta de investigación.

### Comparación

Dos selectores con nombres comprensibles, preguntas compartidas y diferencias. La comparación responde qué objetos estudian, qué problemas resuelven, qué herramientas comparten y cuál es el puente concreto. Las estadísticas de red son una capa adicional y cada valor declara su alcance.

### Recorrido

Objetivo, conocimientos previos, etapas y puntos de comprobación. Cada etapa explica su función. Se permite profundizar sin perder el recorrido original. La ausencia de un puente validado se muestra sin convertirla en una afirmación sobre toda la matemática.

## Ejemplo editorial: Bayes

Propuesta, no contenido ya implementado:

**Objetivo:** actualizar una probabilidad al observar nueva información y distinguir P(A|B) de P(B|A).

**Situación:** se elige la bolsa A con probabilidad 60% y la bolsa C con probabilidad 40%. La bolsa A tiene 80% de bolas rojas; C, 20%. Sale una bola roja. ¿Qué probabilidad hay de que proceda de A?

**Representación:** un árbol o una cuadrícula presenta las proporciones esperadas. De 1000 repeticiones esperadas, 480 combinan A y roja; 80 combinan C y roja. No son resultados garantizados de una muestra real, sino frecuencias esperadas que representan el modelo.

**Resolución:** entre las 560 observaciones rojas esperadas, 480 corresponden a A: 480/560 = 6/7 ≈ 85.7%. Mostrar cómo cada cantidad se corresponde con la fórmula. Definir B como el evento «sale roja» y A como «se eligió la bolsa A».

**Actividad:** antes de recalcular, preguntar qué ocurriría si la bolsa C también tuviera 80% de bolas rojas. Explicar que observar rojo dejaría de distinguir las bolsas y la probabilidad posterior de A seguiría siendo 60%.

**Comprobación:** pedir que el alumno explique por qué 80% no era la respuesta a la pregunta original. Cambiar los valores para comprobar transferencia y abrir después la derivación formal con P(B)>0.

Esto reúne intención, representación, símbolo, acción y explicación. Cada lección requiere revisión antes de publicarse y no todas las matemáticas deben adoptar exactamente el mismo formato.

## Orden de desarrollo

**Primera prioridad:** B03 y E01–E04/E06: homónimos, asociaciones incorrectas, falsa precisión, relaciones y condiciones. Son defectos que pueden enseñar algo falso; una mejora visual no los compensa.

**Después, un conjunto coherente de producto:** entrada por objetivos, búsqueda conceptual, formato de lección, lectura protagonista, mapa contextual y fuentes desplegables. Diseñar juntos A01–A04, B01–B02/B06–B07, C01/C05–C06, D01–D07 y F01–F07 evita que cada pantalla siga una lógica distinta.

**Profundización:** recorridos de estudio, comparación explicativa, aplicaciones y experimentos. Desarrollar modelos editoriales reutilizables para extender estas capacidades al conjunto del atlas, sin exigir que todos los registros estén terminados al mismo tiempo.

**Calidad transversal desde cada entrega:** accesibilidad, localización, semántica, reflujo y pruebas con alumnos. No posponer estas tareas hasta el final. Registrar explícitamente qué regiones tienen material pedagógico revisado y cuáles ofrecen únicamente referencias.

No se estima esfuerzo en días sin leer la arquitectura, el código y el proceso de contenido. La priorización aquí se basa en impacto sobre comprensión y riesgo de interpretación.

## Qué conservar y qué evitar

Conservar la trazabilidad, las URLs de entidades, la distinción de evidencias, la consulta formal, el soporte de teclado anunciado, la alternativa textual y la capacidad de explorar el corpus. La tipografía de lectura y la estética sobria pueden servir como base; necesitan jerarquía y espacio, no una sustitución decorativa automática.

Evitar una capa de chat como sustituto de contenido revisado, animaciones sin propósito, puntos por abrir fichas, porcentajes ficticios de dominio, etiquetas de IA que intenten compensar conexiones erróneas y convertir todos los registros importados en lecciones mediante encabezados vacíos. Mantener la amplitud del atlas sin presentar cobertura documental como cobertura educativa.

## Referencias para los criterios

- [IES: Organizing Instruction and Study to Improve Student Learning](https://ies.ed.gov/ncee/wwc/PracticeGuide/1): ejemplos trabajados alternados con problemas, integración de representaciones concretas y abstractas, preguntas de recuperación y explicación. Sustenta el enfoque didáctico; no garantiza su efectividad específica en MPM.
- [W3C: Understanding Reflow](https://www.w3.org/WAI/WCAG22/Understanding/reflow.html): adaptar el contenido al ancho disponible; las excepciones de contenido bidimensional no convierten toda la página en excepción.
- [W3C: Understanding Non-text Content](https://www.w3.org/WAI/WCAG22/Understanding/non-text-content.html): alternativas para contenido no textual.
- [W3C: Understanding Target Size (Minimum)](https://www.w3.org/WAI/WCAG22/Understanding/target-size-minimum.html): referencia para evaluar controles, considerando las condiciones y excepciones del criterio.

La lista es exhaustiva en dimensiones de presentación cubiertas por esta revisión, no una afirmación de haber encontrado todos los defectos del producto. Se distinguen observaciones, propuestas y pruebas pendientes para no convertir opiniones de diseño en hechos medidos.
