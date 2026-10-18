# Atlas de las conexiones matemáticas

Investigación técnica y programa de construcción de alcance completo

16 de octubre de 2026 · Proyecto personal, abierto y extensible

## La decisión principal

Construir un sistema de conocimiento matemático que pueda recorrerse, interrogarse, verificarse y experimentar con él. La literatura aporta componentes suficientemente desarrollados para empezar una infraestructura extensa. La contribución propia será integrarlos alrededor de conexiones explicables y evaluables, con una experiencia de navegación que conserve el contexto del usuario.

La arquitectura propuesta es un grafo de conocimiento heterogéneo, contextual y versionado, capaz de representar relaciones con varias premisas. Sobre él se construirán un buscador matemático híbrido, un motor de recorridos, vistas multiescala, laboratorios interactivos y herramientas de contribución. Las dependencias formales, las rutas pedagógicas, las analogías y las aplicaciones empíricas se conectarán mediante relaciones explícitas.

La escala objetivo del programa es de millones de enunciados indexados y decenas de millones de relaciones candidatas, acompañados de una capa editorial identificable. El alcance abarca todas las grandes áreas matemáticas desde el diseño inicial y conexiones con ciencias naturales, ingeniería, computación, economía, ciencias sociales, artes y humanidades. La profundidad y la evidencia de cada región serán visibles.

Este documento distingue tres cosas: hallazgos publicados, propuestas de ingeniería derivadas de esos hallazgos y objetivos de investigación propios. Las cantidades, fechas y umbrales del plan son objetivos y supuestos de planificación. No son rendimientos ya obtenidos ni resultados garantizados por los artículos.

## Qué se buscó y qué se verificó

Se realizó una revisión de alcance orientada al diseño, con búsqueda temática, rastreo de antecedentes, consultas por título y contraste en fuentes primarias. Incluye trabajos fundacionales y publicaciones disponibles hasta el 16 de octubre de 2026. La selección reúne 60 referencias y recursos: artículos, preprints, benchmarks, estándares y documentación primaria. No representa 60 artículos leídos íntegramente ni una revisión sistemática exhaustiva.

Las preguntas fueron: cómo representar matemáticas con contexto; cómo extraer relaciones de literatura; cómo conectar objetos formales e informales; cómo recuperar fórmulas y recorridos; cómo visualizar grandes redes; cómo adaptar explicaciones; cómo proponer transferencias entre disciplinas; y cómo mantener procedencia, evaluación y edición abierta.

Se priorizaron métodos originales, datos o implementaciones disponibles, evaluaciones reproducibles y pertinencia directa. Se excluyeron como soporte principal publicaciones promocionales, resultados duplicados, agregadores que no aportan evidencia original y trabajos tangenciales. No se utilizó el número de citas como criterio de verdad ni como ranking del valor de una idea.

La búsqueda web se contrastó con arXiv, ACL Anthology, páginas editoriales y sitios oficiales. Una consulta directa a arXiv recuperó los 33 registros solicitados; Crossref se utilizó para verificar DOI y metadatos editoriales. Una consulta exploratoria a OpenAlex recuperó únicamente la primera página de diez resultados: su total de coincidencias no es el universo elegible de esta revisión. Los registros exactos se conservan en los archivos de búsqueda y recuperación.

Se inspeccionaron secciones del texto completo de TheoremGraph, The Network Structure of Mathlib, Beyond Compilation y GraphRAG; para otras referencias se consultaron resúmenes, metadatos y documentación, según se indica en la matriz. Los preprints recientes se identifican como tales cuando no se confirmó una publicación revisada. La consulta no incluyó una auditoría exhaustiva de retractaciones de todas las referencias.

El buscador Parallel recomendado por las habilidades se instaló temporalmente, pero no tenía autenticación disponible. Se continuó con búsqueda web y APIs bibliográficas públicas. Una descarga directa de ARQMath fue rechazada por el servidor; su página se contrastó mediante el navegador de investigación. No se presentan esos límites de acceso como ausencia de literatura.

## Hallazgos que cambian el diseño

**Hay infraestructura semántica anterior a los LLM que merece reutilizarse.** MMT permite organizar teorías y traducciones sin imponer un único fundamento lógico. Math-in-the-Middle estudia una ontología intermedia para conectar software matemático. sTeX añade semántica al flujo de autoría LaTeX. La propuesta es aprovechar estas ideas para que cada concepto tenga contexto y sus traducciones sean explícitas. [R01](https://arxiv.org/abs/1105.0548) [R02](https://arxiv.org/abs/1603.06424) [R59](https://doi.org/10.47397/tb/43-2/tb134mueller-stex3)

**La extracción extensa ya tiene un precedente muy cercano.** El preprint TheoremGraph reporta 11.7 millones de entornos similares a teoremas y 18.3 millones de dependencias candidatas en el lado informal; también extrae declaraciones y vínculos de proyectos Lean. Sus coincidencias formal/informal incorporan un juez LLM. Es una base para evaluar y reutilizar, con tipos de extracción y procedencia preservados. Su cobertura no equivale a un atlas pedagógico ni a equivalencias formalmente certificadas. [R04](https://arxiv.org/abs/2606.25363)

**La estructura de una biblioteca formal no es directamente la estructura que conviene enseñar.** El análisis de Mathlib distingue dependencias de enunciados, pruebas e infraestructura del lenguaje. Sus resultados justifican investigar una proyección conceptual propia para el atlas, conservando la posibilidad de inspeccionar las dependencias originales. [R06](https://arxiv.org/abs/2604.24797)

**La fidelidad de la traducción necesita una comprobación distinta de la compilación.** Beyond Compilation documenta una brecha entre ambas medidas. Un enunciado bien tipado puede expresar algo diferente al original; una prueba válida certifica el enunciado formal dentro de sus supuestos. El atlas debe registrar por separado traducción, fidelidad y prueba. [R22](https://arxiv.org/abs/2606.31002)

**Las técnicas de navegación multiescala son directamente aplicables.** GraphMaps organiza capas de detalle y mantiene estable la geometría durante la exploración. Ofrece un precedente concreto para una interfaz que no intenta dibujar simultáneamente todo el grafo. [R36](https://arxiv.org/abs/1506.06745)

**La síntesis global y la recuperación local requieren tratamientos distintos.** GraphRAG utiliza comunidades y resúmenes para preguntas panorámicas; RAPTOR organiza resúmenes jerárquicos; HippoRAG explora recuperación mediante recorridos en un grafo. Su combinación es una hipótesis de arquitectura que debe compararse contra alternativas más simples en nuestras tareas. [R29](https://arxiv.org/abs/2404.16130) [R30](https://arxiv.org/abs/2401.18059) [R31](https://arxiv.org/abs/2405.14831)

## Técnicas seleccionadas y uso propuesto

| Necesidad | Técnicas y fuentes | Decisión de diseño |
|---|---|---|
| Representar conceptos y teorías | MMT, Math-in-the-Middle, OpenMath [R01](https://arxiv.org/abs/1105.0548) [R02](https://arxiv.org/abs/1603.06424) [R58](https://openmath.org/standard/) | Objetos contextualizados y alineamientos explícitos entre teorías. |
| Conservar evidencia de cada conexión | Nanopublicaciones, PROV-O, SHACL [R50](https://doi.org/10.3233/ISU-2010-0613) [R51](https://www.w3.org/TR/prov-o/) [R52](https://www.w3.org/TR/shacl/) | Afirmaciones identificables, versionadas y comprobables estructuralmente. |
| Ingerir artículos y libros | unarXive, GROBID, Docling, Nougat, MinerU [R08](https://arxiv.org/abs/2303.14957) [R13](https://grobid.readthedocs.io/en/latest/Introduction/) [R10](https://arxiv.org/abs/2408.09869) [R09](https://arxiv.org/abs/2308.13418) [R11](https://arxiv.org/abs/2509.22186) | Fuentes estructuradas primero; procesamiento visual especializado cuando haga falta. |
| Extraer relaciones científicas | SciERC, SciREX, CitationIE [R14](https://aclanthology.org/D18-1360/) [R15](https://aclanthology.org/2020.acl-main.670/) [R17](https://aclanthology.org/2021.acl-long.59/) | Extraer entidades, contexto y relaciones de varias partes conjuntamente. |
| Interpretar fórmulas | MathAlign, MathBERT, TangentCFT [R18](https://aclanthology.org/2020.lrec-1.269/) [R23](https://arxiv.org/abs/2105.00377) [R24](https://github.com/BehroozMansouri/TangentCFT) | Vincular símbolos con significado local y conservar árboles de expresión. |
| Distinguir usos de publicaciones | SciCite [R16](https://arxiv.org/abs/1904.01608) | Registrar función de la cita y fragmento justificativo. |
| Recuperar conocimiento | SPECTER, SciRepEval, ColBERTv2, ARQMath [R26](https://aclanthology.org/2020.acl-main.207/) [R27](https://aclanthology.org/2023.emnlp-main.338/) [R28](https://aclanthology.org/2022.naacl-main.272/) [R25](https://www.cs.rit.edu/~dprl/ARQMath/) | Búsqueda léxica, semántica y estructural con reranking y pruebas específicas. |
| Conectar con pruebas verificables | Mathlib, LeanDojo, ProofNet [R03](https://arxiv.org/abs/1910.09336) [R19](https://arxiv.org/abs/2306.15626) [R20](https://arxiv.org/abs/2302.12433) | Anclar enunciados a versiones de bibliotecas y entornos de prueba reproducibles. |
| Responder preguntas globales | GraphRAG, RAPTOR, HippoRAG [R29](https://arxiv.org/abs/2404.16130) [R30](https://arxiv.org/abs/2401.18059) [R31](https://arxiv.org/abs/2405.14831) | Orquestador que elige recuperación local, multisalto o panorámica. |
| Navegar grandes redes | GraphMaps, Leiden, representaciones hiperbólicas [R36](https://arxiv.org/abs/1506.06745) [R37](https://arxiv.org/abs/1810.08473) [R39](https://arxiv.org/abs/1705.08039) | Capas, regiones, contexto estable y varias proyecciones. |
| Adaptar el aprendizaje | Knowledge tracing, RefD [R41](https://arxiv.org/abs/1506.05908) [R42](https://doi.org/10.1007/BF01099821) [R43](https://aclanthology.org/D15-1193/) | Perfil voluntario, rutas alternativas y evaluación del aprendizaje. |
| Diseñar experimentos interactivos | Investigación de PhET [R44](https://phet.colorado.edu/en/research) | Simulaciones con preguntas, retroalimentación y entrevistas de uso. |
| Explorar conexiones nuevas | Analogía estructural, descubrimiento en literatura, R-GCN [R45](https://doi.org/10.1016/S0364-0213%2883%2980009-3) [R46](https://doi.org/10.1086/601720) [R47](https://www.nature.com/articles/s41586-019-1335-8) [R57](https://arxiv.org/abs/1703.06103) | Hipótesis de transferencia con condiciones y búsqueda de refutación. |
| Mantener una obra abierta | Wikidata, ORKG, MaRDI, OpenAlex [R49](https://doi.org/10.1145/2629489) [R55](https://arxiv.org/abs/1901.10816) [R54](https://www.mardi4nfdi.de/portal/portal) [R53](https://arxiv.org/abs/2205.01833) | Datos exportables, contribuciones revisables y federación de fuentes. |

## Arquitectura del atlas

La fuente de verdad será un registro de afirmaciones y sus versiones. A partir de él se construirán índices y vistas optimizados. Esto permite sustituir motores de búsqueda, bases de grafos o modelos sin perder la evidencia editorial.

**Objetos fundamentales:** concepto, definición, teoría, enunciado, demostración, conjetura, contraejemplo, algoritmo, modelo, problema, fenómeno, aplicación, publicación, fragmento de fuente, persona, recurso didáctico y experimento. Las traducciones lingüísticas serán representaciones del mismo objeto cuando sea apropiado; las diferencias conceptuales entre tradiciones conservarán identidades separadas y relaciones explícitas.

**Una afirmación de conexión** contendrá participantes con sus roles; relación; dirección; contexto teórico; supuestos; dominio de validez; fuente y localizador; fecha histórica cuando se conozca; fecha de incorporación; extractor y versión; decisiones de revisión; estado; desacuerdos; licencia; y explicación destinada al lector. Las medidas de confianza indicarán qué evento estiman y cómo se calibraron.

**Las relaciones con múltiples premisas** se almacenarán mediante nodos de afirmación o hiperaristas. Si A y B son necesarios para un argumento concreto, el sistema debe preservar esa conjunción. Si existen demostraciones alternativas, también conservará sus alternativas. No se impondrá un único grafo acíclico a toda la base: las citas, analogías y equivalencias tienen propiedades diferentes.

**Cinco proyecciones conectadas:** estructura matemática; dependencias de demostraciones; aprendizaje; aplicaciones y evidencia empírica; historia e influencia. La procedencia y la dimensión temporal atraviesan todas ellas. Una conexión histórica no se convertirá automáticamente en prerrequisito, ni un prerrequisito en necesidad lógica universal.

**Estados públicos de evidencia:** extraído automáticamente; propuesto por un modelo; revisado editorialmente; vinculado a una prueba formal; discutido; sustituido; retirado. Un objeto puede tener varias evaluaciones simultáneas. La etiqueta formal indicará el artefacto que la sustenta y sus axiomas, sin extender esa certificación a una explicación o aplicación no verificada.

**Almacenamiento propuesto:** documentos originales y derivados en almacenamiento de objetos; afirmaciones, revisiones y eventos en una base transaccional; exportaciones columnares para análisis; índice léxico; índice vectorial; índice de estructura de fórmulas; y proyecciones de grafo para consultas. RDF/JSON-LD permitirá intercambio y SHACL comprobará restricciones. La elección concreta de productos se decidirá por pruebas de carga y semántica de consultas, no por popularidad.

## Ingesta y construcción del conocimiento

El flujo completo tendrá diez pasos: inventariar fuentes; verificar identificadores y permisos de uso; preservar versiones y archivos; extraer estructura; reconocer símbolos y contextos; proponer entidades; resolver correspondencias; extraer afirmaciones; evaluar y revisar; publicar e indexar.

Para una fuente LaTeX se conservarán entornos, etiquetas y referencias. Para HTML o XML se aprovecharán sus estructuras. Para PDF se compararán componentes especializados de OCR, diseño, referencias y fórmulas. Cada bloque conservará su posición en la fuente. Se almacenará tanto la notación original como una representación normalizada: el lector debe poder detectar una transcripción defectuosa.

El sistema de extracción separará las menciones de los conceptos. Una letra, un nombre de teorema o un término como «normal» necesita un contexto para desambiguarse. La resolución de entidades usará nombres y alias, definiciones, vecindad, hipótesis, estructura simbólica y evidencia de equivalencia. Los embeddings generarán candidatos; las fusiones exigirán una decisión adicional y serán reversibles.

SciREX y MathAlign aportan precedentes complementarios para relaciones documentales y significado local de símbolos. La adaptación matemática incluirá cuantificadores, dominios, convenciones de notación y referencias a resultados previos. [R15](https://aclanthology.org/2020.acl-main.670/) [R18](https://aclanthology.org/2020.lrec-1.269/)

La revisión combinará validaciones automáticas, verificadores especializados y personas. Se priorizarán casos por incertidumbre, desacuerdo entre extractores, uso esperado y propagación potencial de errores. No se multiplicarán probabilidades de aristas como si todos los errores fueran independientes. Los cambios en una fuente marcarán para revisión sus explicaciones y recorridos derivados.

Desde el comienzo se ingerirán metadatos y estructuras de varias áreas. El grado de profundidad se publicará como cobertura observada por corpus, idioma y tipo de objeto. Una región sin relaciones extraídas se marcará como desconocida o insuficientemente cubierta; no como aislada en las matemáticas.

## Búsqueda y motor de recorridos

El buscador aceptará lenguaje natural, nombres, fórmulas, referencias y combinaciones. Un enrutador determinará si el usuario busca un objeto concreto, una explicación, una ruta educativa, una aplicación o una síntesis de campo. La recuperación combinará señales léxicas, vectores de textos y enunciados, árboles de expresiones y vecindades del grafo.

La fusión de rankings y el reranking se evaluarán contra baselines. El sistema conservará candidatos con notaciones distintas y penalizará incompatibilidades de supuestos. Para resúmenes globales podrá consultar comunidades; para una afirmación específica regresará a las fuentes y a sus relaciones locales. Los resúmenes regionales serán derivados reconstruibles, nunca la única copia del conocimiento.

El motor de recorridos será una de las aportaciones propias. Aplicará reglas de composición de relaciones, objetivos y restricciones. Una cadena de analogías no autoriza a concluir una aplicación demostrada. Una cadena de dependencias de una prueba no determina automáticamente el mejor orden de estudio. Cada recorrido explicará qué autoriza cada paso.

Ofrecerá varias rutas: la que exige menos conocimientos nuevos; la que dispone de mejor respaldo; la que es más intuitiva; la que conserva mayor detalle formal; y la que maximiza diversidad conceptual. Los pesos serán visibles y ajustables. La longitud mínima del camino será una medida disponible, no la definición única de calidad.

La evaluación comprobará pasos, condiciones y resultado del recorrido. También habrá respuestas sin conexión suficiente, búsquedas abiertas y propuestas de investigación. Una recomendación no se convertirá en conocimiento aceptado al ser mostrada repetidamente.

## Una interfaz que permite orientarse y comprender

La vista principal tendrá una región activa, conexiones seleccionadas y un panel de explicación. El usuario podrá ampliar una vecindad, comparar dos regiones, fijar un contexto, retroceder por el historial y abrir una fuente en el fragmento relevante. El mapa completo estará disponible como panorama agregado.

La navegación multiescala utilizará posiciones estables, capas precalculadas y carga por ventana. La agregación conservará el tipo y cantidad de relaciones ocultas. Los enlaces agrupados se podrán descomponer; se evitará que una agrupación gráfica sugiera una relación inexistente. Leiden ayudará a explorar comunidades, con distintas resoluciones y ajustes editoriales explícitos. [R36](https://arxiv.org/abs/1506.06745) [R37](https://arxiv.org/abs/1810.08473)

UMAP y los embeddings hiperbólicos se investigarán como vistas específicas de similitud y jerarquía. La geometría mostrará qué señal representa. No se venderá una proyección bidimensional como distancia matemática objetiva entre disciplinas. [R38](https://arxiv.org/abs/1802.03426) [R39](https://arxiv.org/abs/1705.08039)

Cada objeto ofrecerá intuición, ejemplo, formulación, supuestos, conexiones y fuentes. Las distintas profundidades conservarán anclas comunes. Un usuario podrá abrir la explicación de un símbolo sin abandonar el argumento. La navegación textual y por teclado tendrá el mismo contenido funcional; los experimentos dispondrán de alternativas descriptivas y controles accesibles.

Se diseñará primero alrededor de tareas: localizar una idea, entender una relación, comparar formulaciones, reconstruir una dependencia, encontrar una aplicación y evaluar un recorrido. La tipología de Brehmer y Munzner proporciona una base para especificar esas tareas y medirlas. [R40](https://www.cs.ubc.ca/labs/imager/tr/2013/MultiLevelTaskTypology/)

## Aprendizaje, simulaciones y autoría

El perfil de conocimientos será voluntario y editable. Combinará lo que la persona declara saber con evidencia de ejercicios y explicaciones propias. BKT y DKT servirán como referencias comparativas, y RefD como generador de candidatos de prerrequisitos. La efectividad de las rutas se comprobará mediante aprendizaje y transferencia, además de precisión predictiva. [R41](https://arxiv.org/abs/1506.05908) [R42](https://doi.org/10.1007/BF01099821) [R43](https://aclanthology.org/D15-1193/)

Los laboratorios permitirán manipular hipótesis, observar invariantes, comparar modelos y construir contraejemplos. Cada laboratorio identificará su relación con las fórmulas y resultados del atlas. El motor admitirá componentes reutilizables y ejecución aislada de código, con límites de recursos y versiones de dependencias.

El diseño seguirá ciclos de observación con usuarios inspirados en el trabajo de PhET. Las entrevistas servirán para identificar representaciones que inducen interpretaciones erróneas. La presencia de animación o interactividad no se considerará por sí sola evidencia de aprendizaje. [R44](https://phet.colorado.edu/en/research)

Los autores podrán producir ensayos navegables, anotaciones de libros, recorridos y laboratorios. Una extensión de autoría semántica para LaTeX y Markdown facilitará declarar conceptos y dependencias desde su origen. sTeX ofrece un antecedente particularmente pertinente para este frente. [R59](https://doi.org/10.47397/tb/43-2/tb134mueller-stex3)

## Conexiones con otros campos y descubrimiento

La conexión entre matemáticas y una actividad humana se representará como una cadena de uso documentada: estructura matemática; modelo; procedimiento; implementación o intervención; observación empírica; interpretación. No todas las cadenas tendrán todos los eslabones. El mapa indicará dónde termina la demostración y dónde comienza la evidencia empírica o la interpretación histórica.

Las fichas de aplicación incluirán variables, supuestos, condiciones de validez, métricas observadas y alternativas. Una misma estructura matemática podrá corresponder a varios modelos y escalas. En ciencias sociales, economía y humanidades también se conservarán críticas, desacuerdos de interpretación y límites de medición. Una relación de influencia tendrá una fuente que sostenga esa influencia.

La analogía estructural de Gentner orienta una línea de investigación: emparejar sistemas por relaciones y correspondencias, con diferencias explícitas. Los metacaminos y R-GCN permiten construir candidatos sobre redes heterogéneas. En el atlas serían mecanismos de propuesta, seguidos de contrastación y revisión. [R45](https://doi.org/10.1016/S0364-0213%2883%2980009-3) [R56](https://www.microsoft.com/en-us/research/publication/metapath2vec-scalable-representation-learning-for-heterogeneous-networks/) [R57](https://arxiv.org/abs/1703.06103)

La tradición de descubrimiento basado en literatura y el trabajo de embeddings en materiales justifican explorar puentes poco evidentes. La contribución será evaluar si producen hipótesis útiles para especialistas. La evaluación temporal reconstruirá qué información estaba disponible antes de un descubrimiento, controlando versiones y filtraciones de datos; se añadirán pruebas prospectivas sobre preguntas abiertas. [R46](https://doi.org/10.1086/601720) [R47](https://www.nature.com/articles/s41586-019-1335-8) [R48](https://arxiv.org/abs/2307.15759)

Los primeros conjuntos de recorridos cubrirán varias familias a la vez: simetría y conservación; inferencia e incertidumbre; redes y difusión; optimización y decisiones; geometría y percepción; información y comunicación; dinámica y estabilidad; computación y demostración. Son organizadores editoriales propuestos, no afirmaciones de que toda conexión entre esos campos esté establecida.

## Programa de investigación propio

| Pregunta | Experimento | Resultado que decidiría la implementación |
|---|---|---|
| ¿Cuándo dos enunciados expresan lo mismo? | Recuperación por texto, símbolos y contexto; revisión de pares difíciles; certificados cuando sean posibles. | Precisión de equivalencias y tasa de fusiones falsas por área. |
| ¿Qué reglas permiten componer una ruta válida? | Banco de recorridos correctos y engañosos; ablar tipos, supuestos y contexto. | Validez de cada salto y de la conclusión global. |
| ¿Cómo mantener orientación al cambiar de escala? | Comparar capas estables, layout dinámico y vista textual en tareas equivalentes. | Éxito, tiempo, desorientación y recuperación del contexto. |
| ¿Ayudan las rutas personalizadas a aprender? | Comparar rutas fijas, elegidas por el usuario y adaptativas con medición previa, posterior y diferida. | Comprensión, retención y transferencia con incertidumbre reportada. |
| ¿Son útiles los puentes propuestos por modelos? | Evaluación temporal y juicio ciego de especialistas; seguimiento prospectivo. | Novedad útil, respaldo y carga de revisión. |
| ¿Qué representación explica mejor una conexión? | Comparar explicación textual, visual y manipulable con el mismo contenido. | Calidad de la explicación que produce el usuario y detección de límites. |

## Evaluación: calidad demostrable

Se construirá AtlasBench, una batería pública y versionada. El objetivo inicial de evaluación es reunir 20,000 afirmaciones o pares anotados por especialistas, con solapamiento entre revisores y adjudicación de desacuerdos. Las particiones separarán documentos, familias de resultados, autores cuando proceda y tiempo. Se conservará un conjunto final que no se utilice para seleccionar prompts ni modelos.

| Componente | Medida y objetivo de aceptación propuesto |
|---|---|
| Procedencia | 100% de afirmaciones publicadas con origen, estado y versión identificables. |
| Resolución de entidades | Precisión de fusiones superior a 99% en evaluación estratificada; publicar intervalos y tasa de abstención. |
| Relaciones revisadas | Precisión superior a 97% por familias prioritarias; no usar un promedio global para ocultar clases débiles. |
| Recuperación | Recall y nDCG por texto, fórmulas, idioma y nivel; superar el baseline híbrido según margen predefinido. |
| Respuestas con fuentes | Medir respaldo y cobertura de citas por afirmación, usando ALCE y auditoría humana. [R33](https://arxiv.org/abs/2305.14627) |
| Formalización | Reportar por separado compilación, fidelidad semántica, prueba completa y axiomas. |
| Recorridos | Más de 90% de recorridos seleccionados válidos de extremo a extremo según especialistas; medir también abstenciones. |
| Aprendizaje | Estudio con potencia calculada tras medir variabilidad; no fijar un efecto de aprendizaje sin datos. |
| Interfaz | Objetivo de interacción local p95 menor de 100 ms y búsqueda caliente p95 menor de 2 s, bajo carga y hardware declarados. |
| Accesibilidad y reproducibilidad | Tareas completas por teclado, representación textual equivalente y reconstrucción de una edición del atlas. |

Estos umbrales son decisiones de calidad propuestas. Deben asociarse a tamaños muestrales suficientes, intervalos de confianza y definiciones públicas. La auditoría tendrá ejemplos difíciles, errores de OCR, notaciones ambiguas, cambios de hipótesis y evidencia contradictoria. RAGAS servirá para regresiones frecuentes; SciFact aporta un antecedente para verificar afirmaciones con evidencia. Ninguno reemplaza a los revisores ni al verificador matemático. [R35](https://arxiv.org/abs/2309.15217) [R34](https://arxiv.org/abs/2004.14974)

## Plan de ejecución de alcance completo

El horizonte propuesto es de 18 meses desde el inicio efectivo, con líneas de trabajo simultáneas. Las etapas dependen de artefactos concretos y resultados medidos. El calendario supone capacidad dedicada; no es una estimación para una sola persona trabajando ocasionalmente.

**Meses 1-2: arquitectura y base extensa.** Publicar repositorios, modelo de afirmaciones, esquema de licencias, registro de fuentes, primeras importaciones de varias áreas y diseño de AtlasBench. Comparar extractores sobre un corpus estratificado. Construir el esqueleto visual multiescala con datos reales. La salida es una infraestructura común que ya puede incorporar bibliotecas formales, literatura y conceptos editoriales.

**Meses 2-5: fábrica de conocimiento.** Poner en operación ingesta incremental, conservación de fórmulas, resolución de entidades reversible, extracción contextual y revisión. Integrar resultados formales y los conjuntos externos que permitan sus licencias. Publicar instantáneas descargables y cuadros de cobertura. Meta de capacidad: diez millones de enunciados y cien millones de relaciones candidatas o derivadas, manteniendo su estado de evidencia. Capacidad de ingeniería no significa que todos sean conceptos únicos o contenido revisado.

**Meses 3-7: atlas navegable y búsqueda matemática.** Completar búsqueda híbrida, vistas estructurales e históricas, paneles de conexiones y navegación accesible. La geometría se mantendrá estable dentro de una vista; los cambios entre perspectivas serán explicados. Integrar pruebas de desempeño y estudios de orientación. Salida: una experiencia extensa que permita encontrar y examinar conocimiento real.

**Meses 5-10: explicaciones y aplicaciones.** Producir fichas de conexiones en varios niveles, cadenas de aplicación con evidencia y comparaciones de modelos. Abrir la contribución editorial y la autoría semántica. Incorporar español e inglés como idiomas de primera clase, con revisión de equivalencia y localización.

**Meses 7-13: recorridos, aprendizaje y laboratorios.** Implementar el motor de rutas tipadas, perfiles voluntarios y laboratorios. Evaluar comprensión con personas. Meta editorial del programa: 20,000 fichas conceptuales trabajadas, 100,000 conexiones revisadas, 1,000 recorridos y 120 laboratorios originales o adaptados con permiso. Las variantes automáticas se contarán por separado.

**Meses 10-18: descubrimiento y apertura duradera.** Evaluar propuestas de analogía y transferencia, formalización selectiva y detección de contradicciones o cambios de supuestos. Completar exportaciones, APIs, réplicas y contribuciones externas. Publicar los benchmarks, resultados positivos y negativos y un informe de cobertura. Salida: atlas abierto operable, extensible y auditable, con una agenda de investigación activa.

El camino crítico pasa por identidad y contexto de conceptos, calidad de extracción, procedencia y reglas de composición de recorridos. El renderizado, los laboratorios, la importación bibliográfica y el trabajo editorial pueden avanzar al mismo tiempo, apoyados en contratos estables. Los cambios de esquema tendrán migraciones y versiones.

## Organización y recursos

La propiedad y dirección pueden seguir siendo personales. El volumen propuesto merece una capacidad distribuida de investigación, ingeniería y edición, con colaboradores remunerados y comunidad abierta. Los agentes de IA pueden asumir extracción, pruebas, documentación y producción de candidatos; la responsabilidad de las decisiones científicas y editoriales seguirá siendo identificable.

Como escenario de capacidad, se propone un promedio de 20 equivalentes a tiempo completo durante 18 meses: 1 dirección de producto y arquitectura; 3 ingeniería de datos, búsqueda y plataforma; 3 NLP y grafos; 2 matemáticas formales y semántica; 2 visualización e interacción; 2 aprendizaje y evaluación; y 7 edición matemática y aplicaciones. Las funciones pueden compartirse y variar por etapa. Especialistas externos reforzarán las regiones que lo requieran.

La carga editorial puede cuantificarse. Revisar 100,000 conexiones a cinco minutos por conexión requiere unas 8,333 horas; trabajar 20,000 fichas a treinta minutos requiere otras 10,000 horas. Son supuestos iniciales que hay que medir. Siete personas durante 18 meses a 160 horas mensuales suman 20,160 horas brutas, casi sin margen para coordinación. Por tanto, el programa necesita apoyo editorial externo, reutilización autorizada y revisión por prioridad; no debe ocultar esa diferencia bajo automatización.

| Partida | Escenario de planificación en USD |
|---|---|
| Equipo dedicado | 360 persona-meses × 6,000-12,000 = 2.16-4.32 millones. |
| Cómputo, inferencia, almacenamiento y distribución | Reserva de 0.20-0.80 millones para 18 meses. |
| Especialistas, anotación adicional, traducción y estudios | Reserva de 0.20-0.60 millones. |
| Subtotal | 2.56-5.72 millones. |
| Contingencia de 20% | 0.512-1.144 millones. |
| Total del escenario | 3.072-6.864 millones. |

Estos son supuestos explícitos para dimensionar una ejecución institucionalmente ambiciosa, no cotizaciones de mercado ni un gasto mínimo para empezar. No se han contratado servicios ni comprometido ese presupuesto. Una operación dirigida personalmente con colaboradores y agentes puede distribuir el esfuerzo de otra forma; el costo y el plazo deben recalcularse a partir del rendimiento real, sin asumir que un agente sustituye una especialidad completa.

El costo de inferencia se modelará por tarea: documentos procesados × tokens de entrada y salida × tarifa efectiva × número de pasadas, más reintentos y revisión. Se medirá costo por conexión aceptada y por recorrido útil. La infraestructura distinguirá importación masiva, reconstrucción de índices y servicio interactivo. Los recursos grandes se asignarán a los errores y cuellos de botella demostrados.

## Primeros treinta días: trabajo concreto

1. Publicar la especificación del atlas: preguntas que responde, objetos, relaciones, estados y contratos de API. Definir el registro de fuentes y licencias a nivel de recurso.
2. Evaluar la reutilización de TheoremGraph, Mathlib, unarXive, OpenAlex, MaRDI y ORKG, con identificadores, versiones, cobertura y condiciones documentadas. Preparar adaptadores independientes.
3. Implementar el registro de afirmaciones y evidencia, las migraciones y las exportaciones. Añadir pruebas de conjunciones, alternativas, versiones y cambios de supuestos.
4. Crear una muestra de evaluación estratificada de múltiples áreas, idiomas y formatos; definir instrucciones de anotación y adjudicación. Esta muestra calibra la calidad de la ingesta completa.
5. Comparar extracción estructurada, GROBID, Docling, MinerU y el precedente Nougat por componente. Conservar errores reproducibles de fórmulas y referencias.
6. Construir las tres vías del buscador: léxica, semántica y estructural. Medirlas por separado y combinadas con preguntas reales.
7. Dibujar con datos reales un panorama, una región, una conexión y un recorrido. Implementar navegación textual y accesible en el mismo contrato de datos.
8. Especificar la gramática de recorridos: tipos compatibles, condiciones, premisas conjuntas, alternativas y evidencia faltante. Crear ejemplos adversarios.
9. Preparar herramientas de contribución: propuesta de vínculo, revisión, discusión, historial y autoría de explicaciones. Definir cómo revertir una fusión y propagar correcciones.
10. Publicar un primer informe técnico con cobertura, errores, rendimiento y costos medidos. Utilizarlo para ajustar recursos del programa completo.

## Decisiones que conviene fijar ahora

**Adoptar:** procedencia desde el primer registro; representación de relaciones contextualizadas; almacenamiento independiente de la interfaz; múltiples vistas; evaluación por tarea; exportación; español e inglés; y revisión humana con atribución.

**Comparar experimentalmente:** bases y motores de consulta; encoders y rerankers; extractores visuales; estrategias de síntesis global; modelos de aprendizaje; layouts y métodos de selección de recorridos.

**Investigar como contribuciones originales:** equivalencia contextual entre enunciados; composición de recorridos explicables; proyección conceptual de dependencias formales; calidad pedagógica de las rutas; y transferencia entre disciplinas con evidencia explícita.

La siguiente unidad de trabajo es una especificación ejecutable: esquema de afirmaciones, contratos de ingesta, batería de evaluación y experiencia de navegación. Esa combinación permite construir el atlas completo sobre una base que puede crecer sin perder su significado.

## Documentos que acompañan este informe

La matriz de evidencia relaciona cada referencia con su técnica, aplicación y límite. La bibliografía se entrega en Markdown, JSON y BibTeX. Los registros de búsqueda y recuperación conservan consultas y resultados. El plan de trabajo incluye paquetes con dependencias y criterios de aceptación. Los esquemas visuales sintetizan la arquitectura y el calendario; son diagramas propios, no resultados empíricos.
