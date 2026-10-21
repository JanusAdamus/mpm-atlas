# Piloto Math Coherence Map: conexiones propuestas (sin ingerir)

Fecha: 2026-10-19.  Estado: **para revisión de Adrián**; nada de esto está en el registro.  Fuente: Learning Commons Knowledge Graph v1.15.0 (`relationships.jsonl`, relación `buildsTowards`, autor Student Achievement Partners).  Licencia por fila: CC BY 4.0 de Learning Commons, que recibió las progresiones bajo CC0 (ficha de licencias de docs.learningcommons.org, leída 2026-10-19).  El texto de los estándares no se copia: solo su código.

Cómo se hizo: cada estándar se asignó a mano a **un** concepto del registro a partir de su descripción (juicio de agente, columna «idea»).  Se descartan los pares que caen en el mismo concepto y los repetidos.  Tipo propuesto para todas: «progresión escolar» (`buildsTowards`), que **no** es prerrequisito lógico; la propia documentación advierte que las secuencias no nombran prerrequisitos definitivos.  Si se ingiere, entraría como `prerequisite_for` en `model_proposed` con esa nota, o como relación nueva si el arquitecto la crea.

Total: 44 conexiones.  «Dudosa» = el agente duda de que la conexión entre conceptos sea útil aunque la progresión entre estándares sea correcta.

| # | Desde (estándar) | Hacia (estándar) | Concepto desde | Concepto hacia | Idea desde → idea hacia | Juicio | Id de la relación (Learning Commons) | Revisión AJ |
|---|---|---|---|---|---|---|---|---|
| 1 | 3.NF.A.1 | 4.NF.B.3.a | fracción (`wd:Q66055`) | adición (`wd:Q32043`) | fracción a/b como partes de un todo → suma y resta de fracciones | plausible | `8340a2b4-8879-56fe-abc1-49cad8c7bb8f` |  |
| 2 | 3.NF.A.1 | 5.NF.B.7 | fracción (`wd:Q66055`) | fracción unitaria (`wd:Q255388`) | fracción a/b como partes de un todo → dividir fracciones unitarias | plausible | `a287a888-24b3-525e-b40d-ce05003ee29e` |  |
| 3 | 3.NF.A.2 | 4.NF.B.3.a | recta numérica (`wd:Q35689`) | adición (`wd:Q32043`) | la fracción como número en la recta → suma y resta de fracciones | plausible | `6d1ea36a-0b95-5604-a4b1-212ef274bf3b` |  |
| 4 | 3.NF.A.2 | 6.NS.C.6.a | recta numérica (`wd:Q35689`) | número opuesto (`wd:Q715358`) | la fracción como número en la recta → opuesto de un número | plausible | `af53d140-b6c3-5962-9713-e0cba7accb74` |  |
| 5 | 3.OA.A.1 | 3.OA.A.2 | multiplicación (`wd:Q40276`) | división (`wd:Q1226939`) | interpretar productos → interpretar cocientes | plausible | `ac1891f7-8781-513f-ae0c-458fb2efa3b9` |  |
| 6 | 3.OA.A.1 | 3.OA.B.5 | multiplicación (`wd:Q40276`) | distributividad (`wd:Q187959`) | interpretar productos → propiedades de las operaciones (incluye la distributiva) | plausible | `cd647a9d-5067-58a6-a3c9-c6737dae8138` |  |
| 7 | 3.OA.A.1 | 4.NF.B.4.a | multiplicación (`wd:Q40276`) | fracción unitaria (`wd:Q255388`) | interpretar productos → a/b como múltiplo de 1/b | plausible | `0c62f0cc-9d57-5219-bf77-56f7ba8c6b74` |  |
| 8 | 3.OA.A.1 | 5.NF.B.3 | multiplicación (`wd:Q40276`) | cociente (`wd:Q41118`) | interpretar productos → la fracción como división | plausible | `5c9b160b-36e2-5173-a1de-60fc93f0437a` |  |
| 9 | 3.OA.A.2 | 3.OA.B.5 | división (`wd:Q1226939`) | distributividad (`wd:Q187959`) | interpretar cocientes → propiedades de las operaciones (incluye la distributiva) | plausible | `a7d73ad8-40d1-5864-bb83-8fcc6a40e77b` |  |
| 10 | 3.OA.A.2 | 5.NF.B.3 | división (`wd:Q1226939`) | cociente (`wd:Q41118`) | interpretar cocientes → la fracción como división | plausible | `b962726a-10a6-54d2-800a-54dd7a3b3158` |  |
| 11 | 3.OA.A.2 | 5.NF.B.5 | división (`wd:Q1226939`) | multiplicación (`wd:Q40276`) | interpretar cocientes → multiplicación como escalamiento | plausible | `e54348a2-ed17-58e9-aef2-54c737d1ee42` |  |
| 12 | 3.OA.B.5 | 3.OA.C.7 | distributividad (`wd:Q187959`) | tabla de multiplicar (`wd:Q206049`) | propiedades de las operaciones (incluye la distributiva) → multiplicar y dividir con fluidez hasta 100 | dudosa | `ec933b90-45da-53b3-a96b-74cefe7cee5a` |  |
| 13 | 3.OA.C.7 | 4.OA.B.4 | tabla de multiplicar (`wd:Q206049`) | divisor (`wd:Q50708`) | multiplicar y dividir con fluidez hasta 100 → pares de factores, múltiplos, primos y compuestos | plausible | `5596f4af-b527-547e-91ca-794258ed0ed9` |  |
| 14 | 4.OA.B.4 | 6.EE.A.1 | divisor (`wd:Q50708`) | potenciación (`wd:Q33456`) | pares de factores, múltiplos, primos y compuestos → exponentes enteros | plausible | `2a71c0db-120b-5d64-9bdf-6cf311af7d1d` |  |
| 15 | 4.OA.B.4 | 6.NS.B.4 | divisor (`wd:Q50708`) | máximo común divisor (`wd:Q131752`) | pares de factores, múltiplos, primos y compuestos → máximo común divisor y mínimo común múltiplo | plausible | `0f2d79c4-2fe2-558a-9370-f33488b23e9d` |  |
| 16 | 5.NF.B.3 | 6.RP.A.2 | cociente (`wd:Q41118`) | razón (`wd:Q3481047`) | la fracción como división → tasa unitaria de una razón | dudosa | `acb971e0-bde4-55f2-8971-5e47e572fd2a` |  |
| 17 | 5.NF.B.5 | 6.RP.A.1 | multiplicación (`wd:Q40276`) | razón (`wd:Q3481047`) | multiplicación como escalamiento → razón | plausible | `890a0be1-4595-5f9b-ad5e-795e4a9b419f` |  |
| 18 | 5.NF.B.7 | 6.NS.A.1 | fracción unitaria (`wd:Q255388`) | inverso multiplicativo (`wd:Q216906`) | dividir fracciones unitarias → división de fracciones (recíproco) | plausible | `59bfe4d8-9cfe-5e32-a8a7-c612aa74c5f3` |  |
| 19 | 5.NF.B.7 | 6.RP.A.2 | fracción unitaria (`wd:Q255388`) | razón (`wd:Q3481047`) | dividir fracciones unitarias → tasa unitaria de una razón | plausible | `43600183-55a5-599e-9fd2-4e5e54ef01a4` |  |
| 20 | 5.OA.B.3 | 6.EE.A.2 | sistema de coordenadas cartesianas (`wd:Q62912`) | expresión algebraica (`wd:Q4724001`) | pares ordenados en el plano → expresiones con letras | dudosa | `223259e4-bd81-5575-a4ab-10c2079ff9ae` |  |
| 21 | 5.OA.B.3 | 6.EE.C.9 | sistema de coordenadas cartesianas (`wd:Q62912`) | función lineal (`wd:Q188597`) | pares ordenados en el plano → variable dependiente e independiente | plausible | `4db8aea0-e350-5c94-bfb6-9596f4b949e7` |  |
| 22 | 5.OA.B.3 | 6.RP.A.1 | sistema de coordenadas cartesianas (`wd:Q62912`) | razón (`wd:Q3481047`) | pares ordenados en el plano → razón | plausible | `c1ce8e69-af05-5406-8249-e1615eb81613` |  |
| 23 | 6.EE.A.1 | 6.EE.A.2 | potenciación (`wd:Q33456`) | expresión algebraica (`wd:Q4724001`) | exponentes enteros → expresiones con letras | plausible | `3e7c5613-c3a6-5c84-8ce8-949e9b04a83b` |  |
| 24 | 6.EE.A.2 | 6.EE.A.3 | expresión algebraica (`wd:Q4724001`) | distributividad (`wd:Q187959`) | expresiones con letras → expresiones equivalentes con la propiedad distributiva | plausible | `34bf988a-ba3c-5e08-ac4b-d8230c0763cf` |  |
| 25 | 6.EE.A.2 | 6.EE.B.5 | expresión algebraica (`wd:Q4724001`) | ecuación (`wd:Q11345`) | expresiones con letras → resolver una ecuación o desigualdad | plausible | `48eb3d11-9783-53af-8010-c288aaac705f` |  |
| 26 | 6.EE.A.2 | 6.EE.B.6 | expresión algebraica (`wd:Q4724001`) | variable (`wd:Q50701`) | expresiones con letras → variables | plausible | `13734195-a119-5a8d-9b53-4626efa49426` |  |
| 27 | 6.EE.A.3 | 7.EE.A.1 | distributividad (`wd:Q187959`) | expresión algebraica (`wd:Q4724001`) | expresiones equivalentes con la propiedad distributiva → expresiones lineales | plausible | `ce8cbb6a-c53d-591a-8d7c-d50ce58902df` |  |
| 28 | 6.EE.B.5 | 8.EE.A.2 | ecuación (`wd:Q11345`) | raíz cuadrada (`wd:Q134237`) | resolver una ecuación o desigualdad → raíz cuadrada y cúbica | plausible | `c684b117-5029-5a9f-841b-89b11fe8a95b` |  |
| 29 | 6.EE.B.5 | 8.EE.C.8 | ecuación (`wd:Q11345`) | sistema de ecuaciones lineales (`wd:Q11203`) | resolver una ecuación o desigualdad → sistemas de dos ecuaciones lineales | plausible | `41b1bcce-fb98-5a11-a638-ac3097faa21b` |  |
| 30 | 6.EE.B.6 | 7.EE.B.4.a | variable (`wd:Q50701`) | ecuación (`wd:Q11345`) | variables → ecuaciones px+q=r | plausible | `9ecb9d9b-c171-5434-b932-4c3041065962` |  |
| 31 | 6.EE.B.6 | 7.EE.B.4.b | variable (`wd:Q50701`) | inecuación (`wd:Q165309`) | variables → desigualdades px+q>r | plausible | `040c1087-21d2-56d0-86ce-d1f4c800a94b` |  |
| 32 | 6.NS.A.1 | 6.EE.B.7 | inverso multiplicativo (`wd:Q216906`) | ecuación (`wd:Q11345`) | división de fracciones (recíproco) → ecuaciones x+p=q y px=q | plausible | `889aa431-d5aa-50b6-828e-5e06a90ac637` |  |
| 33 | 6.NS.A.1 | 7.NS.A.2.c | inverso multiplicativo (`wd:Q216906`) | número racional (`wd:Q1244890`) | división de fracciones (recíproco) → multiplicar y dividir racionales | plausible | `fb2da0da-1e0d-5dcb-ad67-cf851c166b2c` |  |
| 34 | 6.NS.B.4 | 6.EE.A.3 | máximo común divisor (`wd:Q131752`) | distributividad (`wd:Q187959`) | máximo común divisor y mínimo común múltiplo → expresiones equivalentes con la propiedad distributiva | plausible | `610aac59-2689-5648-b136-cc8508b99065` |  |
| 35 | 6.NS.C.5 | 6.NS.C.6.a | número negativo (`wd:Q200227`) | número opuesto (`wd:Q715358`) | números positivos y negativos → opuesto de un número | plausible | `85ed77ce-b705-5948-970e-a5e02158e3f6` |  |
| 36 | 6.NS.C.6.a | 6.EE.B.8 | número opuesto (`wd:Q715358`) | inecuación (`wd:Q165309`) | opuesto de un número → desigualdades x>c | plausible | `d2491190-36b5-5b19-abea-4ebd439d9461` |  |
| 37 | 6.NS.C.6.a | 6.NS.C.7.c | número opuesto (`wd:Q715358`) | valor absoluto (`wd:Q120812`) | opuesto de un número → valor absoluto como distancia a 0 | plausible | `aaadd643-e41c-5227-a032-1c12328ce980` |  |
| 38 | 6.RP.A.2 | 6.RP.A.3.c | razón (`wd:Q3481047`) | porcentaje (`wd:Q11229`) | tasa unitaria de una razón → porcentaje | plausible | `78134143-b225-5a19-b2dc-10076649ca8f` |  |
| 39 | 6.RP.A.2 | 7.RP.A.2 | razón (`wd:Q3481047`) | proporcionalidad (`wd:Q51379`) | tasa unitaria de una razón → relaciones proporcionales | plausible | `d8f82542-90b7-5c85-b74e-65b8b83d67aa` |  |
| 40 | 7.EE.B.4.a | 7.EE.B.4.b | ecuación (`wd:Q11345`) | inecuación (`wd:Q165309`) | ecuaciones px+q=r → desigualdades px+q>r | plausible | `f71bd5be-e069-5b67-b2a0-441727d20c33` |  |
| 41 | 7.NS.A.3 | 7.EE.B.4.a | número racional (`wd:Q1244890`) | ecuación (`wd:Q11345`) | problemas con racionales → ecuaciones px+q=r | plausible | `19036297-36db-5529-99e8-a9deddde0a04` |  |
| 42 | 7.NS.A.3 | 8.EE.A.2 | número racional (`wd:Q1244890`) | raíz cuadrada (`wd:Q134237`) | problemas con racionales → raíz cuadrada y cúbica | plausible | `92da377e-93db-5bd0-934d-cfe9b4092cd0` |  |
| 43 | 7.RP.A.2 | 8.EE.B.5 | proporcionalidad (`wd:Q51379`) | pendiente (`wd:Q275447`) | relaciones proporcionales → tasa unitaria como pendiente | plausible | `d7620877-1bd1-5ef3-a5c4-fd99261c60d2` |  |
| 44 | 8.EE.A.1 | 8.EE.A.3 | potenciación (`wd:Q33456`) | notación científica (`wd:Q219142`) | exponentes enteros (negativos incluidos) → potencias de 10 para estimar | plausible | `aa3ff068-5344-5e5f-9fe0-c53d3d854a99` |  |

Generado por `docs/plans/r5/consultas/r5b_coherence_pilot.py` (sin cambios al registro).
