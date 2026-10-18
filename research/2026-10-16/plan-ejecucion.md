# Plan de ejecución

Horizonte: 18 meses desde el inicio efectivo. Objetivos de ingeniería y calidad propuestos; la asignación de recursos se ajustará con mediciones. Las dependencias designan contratos o resultados requeridos, no impiden trabajo preparatorio simultáneo.

## WP01 · Modelo semántico y procedencia

**Responsable:** Arquitectura + semántica. **Ventana:** meses 1-2.

**Dependencias:** ninguna. **Fuentes:** R01, R02, R50, R51, R52.

**Entregable:** Esquema versionado; relaciones n-arias; supuestos; fuentes; historial; validaciones.

**Aceptación:** Todos los ejemplos de evaluación conservan contexto, evidencia y alternativas.

## WP02 · Inventario y acuerdos de reutilización

**Responsable:** Datos + edición. **Ventana:** meses 1-3.

**Dependencias:** ninguna. **Fuentes:** R04, R07, R08, R53, R54.

**Entregable:** Registro por fuente de versión, identificadores, acceso, derechos y cobertura.

**Aceptación:** Cada conjunto incorporado tiene condiciones de uso y procedencia registradas.

## WP03 · Corpus y benchmark de evaluación

**Responsable:** Evaluación + especialistas. **Ventana:** meses 1-6.

**Dependencias:** WP01. **Fuentes:** R12, R20, R25, R33, R34.

**Entregable:** AtlasBench con 20,000 elementos anotados, particiones y adjudicación.

**Aceptación:** Separación temporal/documental; acuerdo entre revisores e incertidumbre reportados.

## WP04 · Ingesta documental

**Responsable:** Ingeniería de datos. **Ventana:** meses 2-5.

**Dependencias:** WP01, WP02. **Fuentes:** R08, R09, R10, R11, R13.

**Entregable:** Adaptadores LaTeX, HTML/XML y PDF; origen de cada bloque.

**Aceptación:** Pruebas por componente, fórmulas y referencias; reconstrucción de una importación.

## WP05 · Extracción y resolución de entidades

**Responsable:** NLP + semántica. **Ventana:** meses 2-8.

**Dependencias:** WP01, WP03, WP04. **Fuentes:** R14, R15, R16, R17, R18, R23.

**Entregable:** Candidatos tipados y fusiones reversibles con evidencia.

**Aceptación:** Precisión por clase; fusiones >99% y abstenciones declaradas en evaluación.

## WP06 · Adaptadores formales

**Responsable:** Matemáticas formales. **Ventana:** meses 2-10.

**Dependencias:** WP01, WP02. **Fuentes:** R03, R04, R06, R19, R20, R22.

**Entregable:** Importación versionada; separación de pruebas, enunciados e infraestructura.

**Aceptación:** Resultados reproducibles; fidelidad y prueba evaluadas de forma independiente.

## WP07 · Recuperación híbrida

**Responsable:** Búsqueda + ML. **Ventana:** meses 3-7.

**Dependencias:** WP03, WP04. **Fuentes:** R23, R24, R25, R26, R27, R28.

**Entregable:** Índices de texto, fórmulas y vectores; ranking y diagnóstico.

**Aceptación:** Evaluación contra baseline por idioma y dominio; p95 caliente <2s en carga declarada.

## WP08 · Mapa multiescala

**Responsable:** Visualización + UX. **Ventana:** meses 2-8.

**Dependencias:** WP01. **Fuentes:** R36, R37, R38, R39, R40.

**Entregable:** Geometría estable, capas, agregación y vistas accesibles.

**Aceptación:** Tareas de orientación evaluadas; interacción local p95 <100ms en hardware declarado.

## WP09 · Síntesis con fuentes

**Responsable:** NLP + edición. **Ventana:** meses 5-10.

**Dependencias:** WP05, WP07. **Fuentes:** R29, R30, R31, R32, R33, R35.

**Entregable:** Explicaciones y resúmenes regionales reconstruibles.

**Aceptación:** Respaldo y cobertura por afirmación; condiciones conservadas y errores auditados.

## WP10 · Motor de recorridos

**Responsable:** Semántica + producto. **Ventana:** meses 5-12.

**Dependencias:** WP05, WP07. **Fuentes:** R01, R15, R31, R43.

**Entregable:** Reglas de composición; rutas alternativas con supuestos y evidencia.

**Aceptación:** Validez de extremo a extremo >90% en conjunto independiente y abstenciones visibles.

## WP11 · Aplicaciones interdisciplinarias

**Responsable:** Edición + especialistas. **Ventana:** meses 4-18.

**Dependencias:** WP01, WP05. **Fuentes:** R16, R34, R45, R46, R47.

**Entregable:** Cadenas modelo-método-uso-evidencia y comparaciones.

**Aceptación:** Afirmaciones empíricas e influencias históricas con fuente propia.

## WP12 · Aprendizaje y laboratorios

**Responsable:** Aprendizaje + visualización. **Ventana:** meses 7-15.

**Dependencias:** WP08, WP10. **Fuentes:** R41, R42, R43, R44.

**Entregable:** Perfiles voluntarios, rutas educativas y 120 laboratorios.

**Aceptación:** Estudio de comprensión, retención y transferencia; simulaciones accesibles.

## WP13 · Descubrimiento de puentes

**Responsable:** ML + especialistas. **Ventana:** meses 10-18.

**Dependencias:** WP05, WP10, WP11. **Fuentes:** R45, R46, R47, R48, R56, R57.

**Entregable:** Candidatos de analogía y transferencia con contrastación.

**Aceptación:** Evaluación temporal y prospectiva; utilidad y carga de revisión publicadas.

## WP14 · Contribución y federación

**Responsable:** Plataforma + comunidad. **Ventana:** meses 4-18.

**Dependencias:** WP01, WP02. **Fuentes:** R49, R50, R51, R54, R55, R59.

**Entregable:** Edición revisable, APIs, exportaciones, réplicas y autoría semántica.

**Aceptación:** Edición reconstruible y exportación completa de datos publicables.

## WP15 · Producción editorial bilingüe

**Responsable:** Edición matemática. **Ventana:** meses 3-18.

**Dependencias:** WP01, WP05, WP14. **Fuentes:** R18, R40, R44, R59, R60.

**Entregable:** 20,000 fichas, 100,000 conexiones revisadas y 1,000 recorridos como metas.

**Aceptación:** Doble control de casos críticos; conteos separados de automático, adaptado y original.

## WP16 · Operación, calidad y reproducibilidad

**Responsable:** Plataforma + evaluación. **Ventana:** meses 1-18.

**Dependencias:** ninguna. **Fuentes:** R12, R25, R33, R35, R48.

**Entregable:** CI de datos; costos; auditorías; backups; pruebas de reconstrucción.

**Aceptación:** Métricas por edición; correcciones propagadas; benchmark y costos publicados.
