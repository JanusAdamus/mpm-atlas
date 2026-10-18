# MPM: auditoría de presentación y aprendizaje

Fecha: 17 de octubre de 2026. Sitio inspeccionado: http://127.0.0.1:8765/#

## Alcance y conclusión

Revisión heurística mediante navegación real: entrada al atlas, búsqueda de derivada, integral y Bayes; fichas de derivada, integración, IsIntegral y teorema de Bayes; cambio a las capas Aprendizaje y Aplicación; ruta derivada → integración con los objetivos Mejor respaldada y Más intuitiva; vista textual; enfoque de región. Se inspeccionaron el contenido accesible y capturas del navegador en su tamaño disponible, aproximadamente 757 × 764 píxeles. No se evaluaron otros tamaños ni se realizó un estudio con alumnos. No se modificó el producto ni se inspeccionó su implementación Rust.

MPM ofrece acceso a un corpus amplio y conserva información de procedencia. En los recorridos observados, la interfaz está organizada alrededor de objetos y conexiones del corpus. Para convertirla en una herramienta pedagógica, necesita una representación editorial de lo que el alumno debe comprender, practicar y conectar. El mapa debe orientar esa actividad y la evidencia debe sustentarla.

## Aspectos que conviene conservar

- Búsqueda accesible, enlaces profundos a entidades, navegación por teclado anunciada y alternativa textual al lienzo.
- Fórmulas renderizadas de forma legible y acciones para situar un concepto en el mapa.
- Distinción visible entre extracción, propuesta de modelo y vinculación formal.
- Fuentes y versiones disponibles para inspección.
- El motor no convierte una cadena bibliográfica en una conclusión matemática: en el caso probado explica por qué la evidencia no basta.

Estas observaciones no constituyen una certificación de accesibilidad ni de exactitud de todo el corpus.

## Hallazgos prioritarios

### H01 · Crítico: una correspondencia semántica equivocada aparece como formalización

**Reproducción:** abrir integración (`#layer=learning&e=wd%3AQ80091`), sección «formalizado como», pulsar `IsIntegral`.

**Observado:** integración enlaza a `mathlib:IsIntegral` con etiqueta «propuesta por modelo» y `p=1.00`. La ficha destino define un elemento de un álgebra que es raíz de un polinomio mónico. La propia evidencia del sitio permite ver que son conceptos diferentes. La fuente declara que las puntuaciones son heurísticas y no calibradas.

**Consecuencia:** la relación y su cifra transmiten una equivalencia que la advertencia posterior no corrige suficientemente. Un alumno puede aprender una asociación errónea.

**Cambio:** retirar esta propuesta de los recorridos pedagógicos y enviarla a revisión. Presentar candidatos como «posible correspondencia pendiente de revisión», sin encabezarlos como una formalización establecida. No representar una puntuación heurística con `p`, ni con porcentajes de certeza. Separar homónimos por dominio, definición y supuestos. Conservar las propuestas en una vista de investigación.

**Aceptación:** integración de cálculo e integralidad algebraica tienen identidades y descripciones diferenciadas; un candidato no revisado no se utiliza como puente didáctico establecido. El caso se incorpora a una colección de regresión semántica.

### H02 · Alta: la entrada no ofrece una primera decisión educativa

**Observado:** el inicio muestra «318.733 nodos» y una agrupación compacta de etiquetas, entre ellas `ISBN`, `Tactic`, `Data/Ordmap`, `Geometry` y `NumberTheory`. No aparece una explicación inicial de tamaños, distancias, colores o selección de etiquetas en la captura inspeccionada.

**Consecuencia:** el principiante debe saber qué buscar antes de poder utilizar la herramienta. La prominencia visual mezcla asuntos matemáticos, documentación e infraestructura.

**Cambio:** conservar la exploración libre y añadir una entrada «¿Qué quieres entender?» con objetivos concretos, búsqueda por preguntas y acceso por áreas comprensibles. Mostrar recorridos sugeridos y permitir indicar conocimientos previos de forma opcional. En el mapa de aprendizaje, priorizar conceptos y objetivos; trasladar módulos y referencias bibliográficas a niveles de detalle apropiados.

**Aceptación:** una persona nueva puede elegir un objetivo y comenzar una explicación sin conocer un identificador, una librería o el nombre técnico exacto. La leyenda aclara qué codifica cada atributo visual y cuándo la proximidad no significa dependencia.

### H03 · Alta: la búsqueda mezcla niveles de abstracción

**Observado:** al buscar `integral`, los primeros resultados son `IsCyclotomicExtension.integral`, `MeasCat.Integral`, `BoxIntegral.integral` y otras declaraciones formales. El concepto `integración` aparece en noveno lugar en la consulta observada. Al buscar `Bayes`, se mezclan persona, teorema, estimadores y declaraciones formales.

**Cambio:** ofrecer resultados agrupados: «Conceptos para aprender», «Teoremas», «Personas e historia», «Formalizaciones» y «Fuentes». En modo de aprendizaje, dar prioridad a coincidencias conceptuales y alias en el idioma del lector. Cada resultado debe incluir una frase de significado y contexto: «Integral — acumulación y área con signo; cálculo». Mantener búsqueda experta para identificadores exactos.

**Aceptación:** `integral` permite encontrar inmediatamente el concepto general; las variantes algebraicas se distinguen explícitamente. El orden no depende exclusivamente de la abundancia de declaraciones de una biblioteca.

### H04 · Alta: «Intuición» contiene descripciones de catálogo

**Observado:** derivada presenta «operación en cálculo infinitesimal»; integración, «operación en cálculo». Bayes presenta una descripción general y después la fórmula, sin ejemplo trabajado en la ficha inspeccionada. No se vieron preguntas de comprensión, práctica o una explicación de cada símbolo en estas fichas.

**Cambio:** distinguir «Descripción de la fuente» de una explicación pedagógica escrita y revisada. Organizar la ficha como: pregunta motivadora → idea central → ejemplo visual y numérico → actividad breve → expresión formal y condiciones → siguiente paso. Permitir saltar directamente a la formulación a quien ya conoce la intuición.

**Aceptación:** una ficha publicada como material de aprendizaje incluye un objetivo observable, ejemplo trabajado, símbolos explicados, condiciones de validez, error frecuente y una pregunta con retroalimentación. Una ficha sin esa elaboración se identifica honestamente como referencia documental.

### H05 · Alta: las etiquetas de relación pierden diferencias importantes

**Observado:** en derivada, «pertenece al área» reúne conexiones procedentes de propiedades distintas, incluidas P279, P31, P2579 y P361. Aparecen entradas repetidas de cálculo diferencial. Las flechas exigen interpretar desde qué entidad se formula la relación. Derivada se etiqueta como «ÁREA».

**Cambio:** preservar el significado de origen y mostrar frases completas desde el punto de vista del lector: «La derivada parcial es un tipo de derivada»; «El cálculo diferencial estudia las derivadas». Distinguir tipo, instancia, parte, campo de estudio, prerrequisito y aplicación. Agrupar afirmaciones equivalentes bajo una explicación conservando todas sus fuentes; no fusionar relaciones diferentes solo porque comparten destino. Revisar la clasificación de derivada.

**Aceptación:** cada conexión puede leerse como una oración inequívoca; una flecha inversa usa la relación inversa adecuada. Los prerrequisitos pedagógicos tienen justificación propia.

### H06 · Alta: una ruta fallida deja al estudiante sin continuación

**Observado:** derivada → integración devuelve «Sin ruta autorizada» con Mejor respaldada y Más intuitiva. Expone una cadena documental a través de Wikipedia y un mensaje con «fuerza 0 < 1». El límite de evidencia es explícito, pero no se ofrece una secuencia de estudio alternativa.

**Cambio:** separar «Camino de conexiones respaldadas» y «Recorrido para aprender». Ante ausencia de camino validado, explicar «MPM todavía no tiene una conexión validada en este corpus» y ofrecer, si existe, una lección editorial revisada o los prerrequisitos disponibles. No inferir que no hay relación matemática. Una ruta didáctica debe tener objetivos por etapa, actividades y verificación de comprensión; su calidad no se reduce a longitud o fuerza de aristas.

**Aceptación:** los fallos distinguen ausencia de cobertura, evidencia insuficiente y requisitos del recorrido. Cada estado tiene un siguiente paso útil sin inventar conexiones.

### H07 · Media-alta: las capas no comunican suficientemente qué cambia

**Observado:** cambiar derivada de Estructura a Aprendizaje conserva el contenido de la ficha. En Aplicación, Bayes enlaza a estadística bayesiana y probabilidad bayesiana, pero la ficha no desarrolla un caso aplicado. No se concluye que las capas no filtren el mapa: el hallazgo se refiere al contenido educativo visible de la ficha.

**Cambio:** explicar la pregunta de cada vista: «¿Cómo se relaciona?», «¿Qué necesito aprender antes?», «¿Dónde se usa?», «¿Cómo se demuestra?» y «¿Cómo surgió?». Mantener visible la vista activa. En aplicaciones, narrar problema → modelo → cálculo → interpretación → límites. En aprendizaje, mostrar conocimientos previos, siguiente concepto y actividad.

**Aceptación:** el usuario identifica qué cambia al alternar vistas; los estados sin contenido específico se explican y no aparentan una lección completa.

### H08 · Media-alta: la vista textual expone el mismo exceso de información

**Observado:** Aprendizaje muestra 75 regiones y 325 entidades visibles. La lista incluye fuentes como ISBN, Estados Unidos, October 17 y Parables of Jesus, junto con conceptos matemáticos. El mapa enfocado en Bayes conserva etiquetas ajenas al objetivo inmediato, incluidas módulos de álgebra de Lie y entradas de otros temas.

**Cambio:** diseñar la vista textual como un índice pedagógico propio: objetivo actual, conceptos principales, conceptos previos, conexiones explicadas y fuentes bajo demanda. Mostrar alrededor del concepto activo un pequeño vecindario seleccionado y permitir expandirlo. El mapa global permanece disponible para exploración.

**Aceptación:** se puede entender una conexión y elegir el siguiente paso sin recorrer cientos de entradas. El atlas conserva la amplitud de datos y hace explícita la selección editorial de lo que se muestra.

### H09 · Media: el detalle técnico desplaza el contenido de aprendizaje

**Observado:** los identificadores, alias multilingües, enlaces de procedencia, puntuaciones y textos técnicos en inglés ocupan parte sustancial de las fichas. En IsIntegral, «SUPUESTOS» incluye mensajes del proceso de extracción como «axioms not computed», diferentes de hipótesis matemáticas. En el ancho examinado, mapa y ficha se apilan y compiten por la altura disponible.

**Cambio:** ofrecer un modo de lectura con mapa contextual plegable y un índice local. Mantener la atribución necesaria, pero colocar identificadores, versiones, importación y alias extensos bajo «Evidencia y detalles». Separar «Hipótesis matemáticas» de «Estado de verificación». Traducir las explicaciones y señalar el idioma original de lo que aún no tiene traducción.

**Aceptación:** la primera pantalla de una lección muestra objetivo e idea central; se puede acceder a todas las fuentes sin tener que atravesarlas para aprender. No se confunden límites de extracción con condiciones de un teorema.

## Ejemplo de ficha propuesta: derivada

**Título:** Derivada: medir cómo cambia algo en un instante.

**Objetivo:** interpretar una derivada como tasa local de cambio y pendiente, y distinguirla de una tasa media.

**Pregunta de entrada:** si la posición de un objeto cambia con el tiempo, ¿cómo describimos su velocidad en un instante?

**Idea central:** cerca de un punto donde una función es derivable, su gráfica se aproxima mediante una recta. La pendiente de esa recta describe cuánto cambia la salida ante un cambio pequeño de la entrada.

**Experimento:** mostrar f(x)=x² alrededor de x=2. El alumno modifica h y observa dos puntos, una secante y el cociente ((2+h)²−4)/h=4+h para h≠0. Al acercar h a cero, el cociente se acerca a 4. Un control equivalente debe funcionar con teclado, valores numéricos y descripción textual.

**Pregunta antes de revelar:** ¿qué esperas que ocurra con la pendiente al reducir h a la mitad? Pedir una predicción y explicar la respuesta; no limitarse a animar la gráfica.

**Formalización:** presentar el límite después del ejemplo, explicar f, x y h y señalar que la derivada existe cuando el límite pertinente existe. Mostrar |x| en cero como contraste: pendientes laterales −1 y 1, sin derivada bilateral.

**Comprobación:** pedir que el alumno distinga el valor f(2)=4 de la pendiente f′(2)=4; después cambiar a x=3 para evitar que la coincidencia numérica oculte la diferencia. Preguntar qué representa una pendiente negativa.

**Conexiones seleccionadas:** pendiente → tasa media → límite → derivada → optimización/velocidad. Explicar la función de cada paso. Un recorrido hacia integración debería desarrollar el teorema fundamental del cálculo con sus condiciones, no usar enlaces documentales como sustituto del argumento.

**Evidencia:** la lección y sus conexiones necesitan revisión matemática y fuentes apropiadas antes de publicarse. Este ejemplo es una propuesta editorial, no contenido ya implementado.

## Secuencia de mejora del producto

1. **Corregir lo que puede enseñar algo falso.** Resolver la asociación integración/IsIntegral; revisar homónimos; corregir tipos y nombres de relaciones; separar candidatos de correspondencias establecidas; eliminar la apariencia probabilística de puntuaciones no calibradas.
2. **Construir el modelo de lección y búsqueda educativa.** Definir objetivos, nivel, conocimientos previos, explicaciones, ejemplos, actividades, errores frecuentes, fuentes y revisión como datos de contenido. Priorizar conceptos y ofrecer desambiguación.
3. **Reorganizar la experiencia principal.** Inicio por objetivos, lectura centrada en la lección, mapa contextual, evidencia desplegable y estados vacíos orientadores. Mantener la exploración avanzada accesible.
4. **Crear recorridos de aprendizaje.** Secuencias editoriales revisadas, etapas con objetivos y preguntas, alternativas según conocimientos y registro voluntario del avance. No equiparar abrir una ficha con dominar el concepto.
5. **Desarrollar aplicaciones y experimentos.** Casos con variables, supuestos, cálculos e interpretación, conectados al atlas. Incluir explicación textual y controles accesibles para cada interacción.
6. **Evaluar con estudiantes y docentes.** Ajustar presentación, vocabulario y actividades según tareas observadas y comprensión, no solo clics o tiempo de permanencia.

El alcance puede extenderse a todas las áreas del atlas. El modelo editorial y la navegación deben permitir esa extensión sin exigir que cada entrada documental se presente como una lección terminada. La cobertura pedagógica publicada debe distinguirse de la cobertura bibliográfica.

## Criterios de éxito sugeridos

Son criterios propuestos, no resultados medidos ni umbrales validados:

- Una persona nueva puede explicar para qué sirve MPM y comenzar un objetivo sin asistencia.
- Una búsqueda cotidiana lleva a un concepto comprensible y distingue sus homónimos.
- El alumno explica una conexión con sus propias palabras y no interpreta una cita como demostración.
- Después de una lección resuelve un problema nuevo, además del ejemplo mostrado.
- Una comprobación diferida permite evaluar retención, no solo reconocimiento inmediato.
- Cada conexión editorial tiene evidencia y revisión; cada candidata aparece como tal.
- Las actividades se pueden recorrer con teclado y tienen alternativas textuales y simbólicas.

Comparar la presentación actual con la propuesta mediante tareas equivalentes, incluyendo estudiantes con diferentes conocimientos previos. Observar bloqueos, errores de interpretación, explicación de símbolos, transferencia y recuerdo. Registrar también dónde el sistema aún carece de material.

## Fundamento pedagógico y límites

La guía primaria del Institute of Education Sciences recomienda alternar ejemplos resueltos con problemas, integrar representaciones concretas y abstractas y utilizar preguntas de recuperación para apoyar el aprendizaje. Estas recomendaciones sustentan el formato propuesto; su efectividad específica en MPM debe evaluarse.

Fuente: [Organizing Instruction and Study to Improve Student Learning](https://ies.ed.gov/ncee/wwc/PracticeGuide/1).

Esta auditoría se basa en una muestra explícita de pantallas y recorridos. No demuestra la frecuencia global de los errores ni la inexistencia de funciones no visitadas. La asociación incorrecta, las frases citadas y los resultados de ruta descritos sí se observaron directamente en el despliegue.
