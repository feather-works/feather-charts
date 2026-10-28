# feather-charts

Motor de gráficos **100% Rust** da suíte **FeatherWorks**: o mesmo gráfico
no FeatherSheet (gráfico solto na planilha), no FeatherDocs (gráfico no texto)
e no FeatherSlide (gráfico no slide), com a mesma geometria, a mesma aparência
e a mesma exportação.

É uma **biblioteca**, não um aplicativo e **não tem interface Iced**: cada
aplicativo desenha o resultado do jeito que sabe. A `feather-charts` recebe
dados já prontos, calcula a geometria e entrega uma `Cena` do
[`feather-draw`](https://github.com/feather-works/feather-draw), que sai em **tela**, **SVG** e **PDF** a partir
das mesmas formas.

> **Estado do documento.** Este README é a **especificação de suporte**: lista
> todos os gráficos e todas as formas de personalização que a biblioteca vai
> ter. A meta é **100% personalizável**: nada de aparência fixa dentro do
> motor. Cada item traz o estado real do código deste repositório (extraído
> em 14/10/2026 do monorepo FeatherWorks, onde se chamava `featheroffice-charts`).
>
> | Marca | Significado |
> |---|---|
> | Pronto | existe no código atual e tem teste |
> | Parcial | a geometria básica existe, faltam eixos, rótulos, estilo ou integração |
> | Planejado | ainda não existe |
>
> **Oficial desde 15/10/2026:** este repositório é a versão oficial do motor de
> gráficos da suíte. A cópia `featheroffice-charts` do monorepo deixou de
> evoluir; o que ela ganhou depois da extração veio para cá: o modelo de estilo
> serializável do painel "Formatar gráfico" (`estilo`: `EstiloGrafico`, 10
> temas em `estilo::temas()`) e o desenho formatado (`formatado`), além do
> título e da legenda que não pisam nos rótulos do eixo (`layout_anotado`).
> FeatherSheet, FeatherDocs, FeatherSlide e FeatherPDF dependem daqui.

## Por que existe

Cada aplicativo da suíte precisava de gráficos, e a tentação era escrever um
renderizador em cada um. O resultado seria o que já aconteceu com a interface:
três gráficos de colunas diferentes, três jeitos de formatar número, e um
slide que não bate com a planilha de onde veio o dado.

A `feather-charts` existe para três coisas:

1. **Um gráfico, três aplicativos.** O mesmo dado e a mesma aparência geram o
   mesmo desenho na planilha, no documento e no slide, e no PDF impresso.
2. **Personalização total.** Tudo que o Excel e o PowerPoint deixam mudar
   (e o que eles não deixam) é uma propriedade declarativa, salva no arquivo
   do aplicativo, sem depender de nenhum código dentro do motor.
3. **Privacidade e independência.** O motor não faz acesso à rede, não tem
   telemetria e não conhece planilhas: quem lê o intervalo, agrega ou filtra
   é o aplicativo. Nenhum dado do usuário sai da máquina.

## Decisões de projeto

- **Rust puro**, sem biblioteca em C por trás. A única dependência de peso é o
  `feather-draw`, que também é Rust puro.
- **Sem Iced.** A saída é uma `Cena` (formas vetoriais) mais uma lista de
  rótulos de texto (`CenaAnotada`). Iced, SVG e PDF são adaptadores.
- **Dados já prontos.** O motor nunca recalcula soma, média ou agrupamento
  (a exceção são cálculos que fazem parte do tipo, como as caixas do
  histograma, o saldo da cascata e as linhas de tendência). Quem entrega a
  série é a origem: Sheet, tabela do Docs ou grade do Slide.
- **Os valores nunca são alterados.** Amostragem, ordenação visual e
  arredondamento acontecem só no desenho; a série original sai intacta.
- **Determinismo.** Os mesmos dados e a mesma aparência produzem exatamente a
  mesma `Cena` e o mesmo SVG, byte a byte (os testes comparam).
- **Cores só como dados.** O motor não tem cor fixa escondida: a paleta, o
  contorno e a opacidade vêm da aparência (padrão = paleta da suíte).
- **Português do Brasil por padrão**, com vírgula decimal e datas pt-BR;
  en-US disponível.

## O que existe hoje (resumo honesto)

O crate de origem tem duas camadas:

- **Camada `Grafico` + `AparenciaGrafico`**: 7 tipos (`Tipo`: colunas, barras,
  linhas, área, degraus, pizza, dispersão), com eixos e ticks calculados,
  paleta, opacidade, contorno, marcadores, margens, LTTB, tipografia
  (título, subtítulo, unidade, formato numérico pt-BR/en-US, rótulos),
  interação (hover, clique, seleção, edição de valor), exportação SVG e PDF
  e o contrato de dados `ContratoGrafico`. É a camada estável, usada pelo
  Sheet, pelo Docs (SVG embutido no texto) e pelo Slide.
- **Camada `motor`**: 25 identificadores (`TipoGrafico`, ids herdados do
  antigo `chart.ts`: `column`, `bar`, `line`, `waterfall`…), cada um em
  um módulo próprio. Desenham a **geometria básica** (sem eixos, sem
  rótulos, sem aparência configurável). Serve como base para promover cada
  tipo à camada completa; por isso quase todos aparecem como Parcial abaixo.

A migração planejada junta as duas camadas num só modelo (`Grafico` com um
`TipoGrafico` e propriedades de aparência por elemento).

---

## 1. Catálogo de gráficos

Legenda da última coluna (**OOXML**): como o tipo é gravado em `.xlsx`,
`.pptx` e `.docx`.

- **nativo**: existe elemento próprio do DrawingML (`c:` = `chart`, em
  `drawingml/2006/chart`). A biblioteca grava o gráfico editável.
- **chartex**: gráfico "novo" do Office 2016+ (`cx:`, parte `chartEx`).
- **combinação**: formado por tipos nativos combinados (ex.: cascata antiga =
  barras empilhadas com série invisível).
- **imagem/formas**: sem equivalente editável; vai como SVG/PNG ou grupo de
  formas (`p:grpSp`/`a:grpSp`) preservando o desenho, mas não a edição.

### 1.1 Colunas e barras

| Id | Gráfico (pt-BR) | Inglês | Variantes | Estado | OOXML |
|---|---|---|---|---|---|
| T01 | Colunas agrupadas | Clustered column | uma ou várias séries, valores negativos, sobreposição e espaçamento | Pronto | nativo `c:barChart` (`barDir=col`, `grouping=clustered`) |
| T02 | Colunas empilhadas | Stacked column | positivos e negativos separados | Parcial | nativo `c:barChart` (`grouping=stacked`) |
| T03 | Colunas 100% empilhadas | 100% stacked column | proporção por categoria | Parcial | nativo (`percentStacked`) |
| T04 | Barras agrupadas | Clustered bar | horizontal, categorias no eixo Y | Pronto | nativo `c:barChart` (`barDir=bar`) |
| T05 | Barras empilhadas | Stacked bar | positivos e negativos separados | Parcial | nativo |
| T06 | Barras 100% empilhadas | 100% stacked bar | proporção por categoria | Parcial | nativo |
| T07 | Colunas 3D | 3-D column | agrupada, empilhada, 100%, 3D total (profundidade) | Planejado | nativo `c:bar3DChart` |
| T08 | Barras 3D | 3-D bar | agrupada, empilhada, 100% | Planejado | nativo `c:bar3DChart` |
| T09 | Cilindro, cone e pirâmide | Cylinder, cone, pyramid | formas 3D sobre colunas e barras | Planejado | nativo (`c:shape`) |
| T10 | Colunas de intervalo (flutuantes) | Floating / range column | início e fim por categoria | Planejado | combinação (empilhada com série sem preenchimento) |
| T11 | Barras divergentes | Diverging stacked bar | escala Likert, pirâmide etária | Planejado | combinação |
| T12 | Barras com largura variável (Marimekko) | Marimekko / variable-width column | largura = peso da categoria | Planejado | imagem/formas |
| T13 | Pirulito | Lollipop | haste + ponto, vertical ou horizontal | Planejado | combinação (barras de erro + marcadores) |
| T14 | Haltere | Dumbbell / dot plot de duas séries | dois pontos ligados por segmento | Planejado | combinação (linhas de alta e baixa) |
| T15 | Bala | Bullet chart | valor, meta e faixas de qualidade | Planejado | combinação |
| T16 | Régua de progresso | Progress bar | barra única 0-100%, vários itens | Planejado | combinação |
| T17 | Pictograma | Pictogram / isotype | ícone repetido ou barra preenchida por figura | Planejado | imagem/formas |
| T18 | Waffle | Waffle chart | grade 10x10 de quadrados, proporção | Planejado | imagem/formas |
| T19 | Colunas com subcategorias | Multi-level category column | eixo com categorias em dois ou mais níveis | Planejado | nativo (`c:multiLvlStrRef`) |

### 1.2 Linhas

| Id | Gráfico (pt-BR) | Inglês | Variantes | Estado | OOXML |
|---|---|---|---|---|---|
| T20 | Linhas | Line | sem marcadores | Pronto | nativo `c:lineChart` |
| T21 | Linhas com marcadores | Line with markers | quadrado e círculo prontos; demais formas planejadas | Parcial | nativo (`c:marker`) |
| T22 | Linhas empilhadas e 100% | Stacked / 100% stacked line | acumulado por categoria | Parcial | nativo (`stacked`, `percentStacked`) |
| T23 | Linhas suavizadas | Smoothed line | curva spline (Catmull-Rom/monótona) | Planejado | nativo (`c:smooth`) |
| T24 | Degraus (escada) | Step line | horizontal e depois vertical; empilhado e 100% | Pronto (simples) / Parcial (empilhado, 100%) | combinação (sem tipo nativo) |
| T25 | Linha 3D (XYZ) | 3-D line / ribbon | projeção isométrica de X, Y e Z | Parcial | nativo `c:line3DChart` (versão da fita) ou imagem/formas |
| T26 | Série temporal com eixo de datas | Date-axis line | espaçamento proporcional ao tempo, unidade base dia/mês/ano | Planejado | nativo (`c:dateAx`) |
| T27 | Linha de inclinação | Slope chart | dois instantes, uma linha por item | Planejado | nativo (linhas, duas categorias) |
| T28 | Linha do tempo | Timeline | marcos e períodos sobre um eixo de datas | Planejado | imagem/formas |
| T29 | Linhas com lacunas | Line with gaps | dado ausente: lacuna, zero ou interpolar | Planejado | nativo (`c:dispBlanksAs`) |

### 1.3 Área

| Id | Gráfico (pt-BR) | Inglês | Variantes | Estado | OOXML |
|---|---|---|---|---|---|
| T30 | Área | Area | fecha na linha do zero real, mesmo com negativos | Pronto | nativo `c:areaChart` |
| T31 | Área empilhada e 100% | Stacked / 100% stacked area | acumulado e proporção | Parcial | nativo (`stacked`, `percentStacked`) |
| T32 | Área em degraus | Step area | simples, empilhada, 100% | Parcial | combinação |
| T33 | Área 3D | 3-D area | agrupada, empilhada, 100%, 3D total | Planejado | nativo `c:area3DChart` |
| T34 | Área em fluxo | Streamgraph | áreas empilhadas com base ondulante (simétrica) | Planejado | imagem/formas |
| T35 | Faixa (intervalo) | Range / band area | área entre dois limites, com linha central | Planejado | combinação (área empilhada com série transparente) |

### 1.4 Pizza e proporção

| Id | Gráfico (pt-BR) | Inglês | Variantes | Estado | OOXML |
|---|---|---|---|---|---|
| T36 | Pizza | Pie | rótulos com texto reduzido, fatias nulas sem área, valores muito grandes | Pronto | nativo `c:pieChart` |
| T37 | Rosca | Doughnut | furo configurável, várias anéis (uma série por anel), texto no centro | Parcial (furo e um anel por série; sem texto no centro) | nativo `c:doughnutChart` |
| T38 | Pizza explodida | Exploded pie | fatia afastada, todas ou uma só | Pronto (`EstiloPizza::explodir` por fatia e `explodir_todas`) | nativo (`c:explosion`) |
| T39 | Pizza 3D | 3-D pie | inclinação e rotação | Planejado | nativo `c:pie3DChart` |
| T40 | Pizza de pizza | Pie of pie | fatias pequenas viram uma segunda pizza | Planejado | nativo `c:ofPieChart` (`ofPieType=pie`) |
| T41 | Barra de pizza | Bar of pie | fatias pequenas viram uma barra empilhada | Planejado | nativo `c:ofPieChart` (`ofPieType=bar`) |
| T42 | Semipizza e medidor circular | Half pie / gauge ring | ângulo inicial e varredura livres | Planejado | combinação (rosca com fatia invisível) |
| T43 | Mapa de árvore | Treemap | retângulos proporcionais, hierarquia, cores por nível | Planejado | chartex `cx:chart layoutId="treemap"` |
| T44 | Explosão solar | Sunburst | anéis hierárquicos | Planejado | chartex `layoutId="sunburst"` |
| T45 | Funil | Funnel | etapas decrescentes, centralizado | Planejado | chartex `layoutId="funnel"` |
| T46 | Pirâmide | Pyramid | níveis proporcionais | Planejado | imagem/formas |
| T47 | Círculos agrupados | Circle packing | hierarquia em círculos | Planejado | imagem/formas |

### 1.5 Dispersão, bolhas e relação

| Id | Gráfico (pt-BR) | Inglês | Variantes | Estado | OOXML |
|---|---|---|---|---|---|
| T48 | Dispersão (XY) | Scatter | só marcadores | Pronto | nativo `c:scatterChart` |
| T49 | Dispersão com linhas | Scatter with lines | retas ou suavizadas, com e sem marcadores | Planejado | nativo (`scatterStyle`) |
| T50 | Bolhas | Bubble | tamanho pela segunda série ou `z_values`; área proporcional | Parcial | nativo `c:bubbleChart` |
| T51 | Bolhas 3D | 3-D bubble | efeito de esfera | Planejado | nativo (`c:bubble3D`) |
| T52 | Nuvem de pontos com regressão | Scatter with regression | tendência, banda de confiança, equação e R² | Planejado | nativo (`c:trendline`) |
| T53 | Dispersão com erro (cruz) | XY with error bars | barras de erro em X e Y | Planejado | nativo (`c:errBars`) |
| T54 | Matriz de dispersão | Scatter matrix / SPLOM | todas as colunas contra todas | Planejado | imagem/formas |
| T55 | Densidade 2D / hexbin | 2-D density / hexbin | pontos agregados em células | Planejado | imagem/formas |
| T56 | Diagrama de Sankey | Sankey | fluxos entre etapas, largura proporcional | Planejado | imagem/formas |
| T57 | Diagrama de cordas | Chord diagram | relações em círculo | Planejado | imagem/formas |
| T58 | Coordenadas paralelas | Parallel coordinates | uma linha por registro | Planejado | imagem/formas |
| T59 | Rede | Network / node-link | nós e arestas, layout em força ou circular | Planejado | imagem/formas |
| T60 | Organograma / árvore | Org chart / tree | hierarquia | Planejado | imagem/formas (no Slide, ver SmartArt do app) |

### 1.6 Distribuição e estatística

| Id | Gráfico (pt-BR) | Inglês | Variantes | Estado | OOXML |
|---|---|---|---|---|---|
| T61 | Histograma | Histogram | hoje: caixas = raiz do total (máx. 32); planejado: largura, número, limites e overflow/underflow configuráveis | Parcial | chartex `layoutId="clusteredColumn"` (com `binning`) |
| T62 | Pareto | Pareto | colunas em ordem decrescente + linha de % acumulado | Planejado | chartex `layoutId="pareto"` |
| T63 | Caixa e bigode | Box and whisker | quartil inclusivo/exclusivo, média, outliers | Planejado | chartex `layoutId="boxWhisker"` |
| T64 | Violino | Violin | densidade espelhada | Planejado | imagem/formas |
| T65 | Densidade (KDE) | Density / ridgeline | curva suave, vários grupos empilhados | Planejado | imagem/formas |
| T66 | Faixa de erro (intervalo) | Error band / interval | média com limite inferior e superior | Planejado | combinação |
| T67 | Dispersão em faixa (strip/swarm) | Strip / beeswarm | pontos por categoria sem sobreposição | Planejado | nativo (dispersão) |
| T68 | Q-Q e probabilidade normal | Q-Q plot | comparação com distribuição teórica | Planejado | nativo (dispersão) |
| T69 | Controle (CEP) | Control chart | linha central, limites superior e inferior | Planejado | nativo (linhas) |

### 1.7 Comparação radial

| Id | Gráfico (pt-BR) | Inglês | Variantes | Estado | OOXML |
|---|---|---|---|---|---|
| T70 | Radar (teia) | Radar | linhas; preenchido e com marcadores planejados | Parcial | nativo `c:radarChart` (`standard`, `marker`, `filled`) |
| T71 | Polar / rosa dos ventos | Polar area / wind rose | setores com raio proporcional, direções cardeais | Planejado | imagem/formas |
| T72 | Barras radiais | Radial bar | barras em arco concêntrico | Planejado | imagem/formas |
| T73 | Velocímetro (medidor) | Gauge / speedometer | ponteiro, faixas verde-amarelo-vermelho, meta | Planejado | combinação (rosca + pizza) |
| T74 | Cartão de indicador (KPI) | KPI card | valor, variação, seta, minigráfico | Planejado | imagem/formas |

### 1.8 Mercado financeiro

| Id | Gráfico (pt-BR) | Inglês | Variantes | Estado | OOXML |
|---|---|---|---|---|---|
| T75 | Velas (candlestick) | Candlestick (OHLC) | abertura, máxima, mínima e fechamento; cor de alta/baixa; hoje valida os dados | Parcial | nativo `c:stockChart` |
| T76 | Barras OHLC | OHLC bars | traços de abertura e fechamento | Planejado | nativo `c:stockChart` |
| T77 | Máxima-mínima-fechamento | High-low-close | HLC | Planejado | nativo `c:stockChart` |
| T78 | Volume, abertura, máx., mín., fech. | Volume-OHLC | volume com eixo secundário | Planejado | nativo (`c:stockChart` + barras) |
| T79 | Renko / Heikin-Ashi / Ponto e figura | Renko, Heikin-Ashi, P&F | derivados de OHLC | Planejado | imagem/formas |

### 1.9 Combinados, cascata e comparativos

| Id | Gráfico (pt-BR) | Inglês | Variantes | Estado | OOXML |
|---|---|---|---|---|---|
| T80 | Combinado | Combo | colunas na primeira série e linhas nas demais; planejado: tipo por série | Parcial | nativo (vários `c:*Chart` no `c:plotArea`) |
| T81 | Combinado com eixo secundário | Combo with secondary axis | séries em escalas diferentes | Planejado | nativo (dois `c:valAx`) |
| T82 | Cascata | Waterfall | hoje: incrementos sobre o saldo, verde e vermelho; planejado: totais, subtotais, conectores | Parcial | chartex `layoutId="waterfall"` |
| T83 | Ponte (bridge) de variação | Variance bridge | do valor inicial ao final por fatores | Planejado | chartex (cascata) |
| T84 | Gantt (cronograma) | Gantt | hoje: início e fim numéricos; planejado: datas, dependências, marco, progresso, hoje | Parcial | combinação (barras empilhadas com série invisível) |
| T85 | Mapa de calor | Heatmap | hoje: gradiente azul-vermelho global; planejado: escala configurável, calendário | Parcial | imagem/formas (ou formatação condicional da célula) |
| T86 | Mapa de calor de calendário | Calendar heatmap | dias do ano em grade semanal | Planejado | imagem/formas |
| T87 | Quadros pequenos (small multiples) | Small multiples / facets | mesmo gráfico repetido por categoria, escalas compartilhadas | Planejado | imagem/formas |

### 1.10 Superfície e 3D

| Id | Gráfico (pt-BR) | Inglês | Variantes | Estado | OOXML |
|---|---|---|---|---|---|
| T88 | Superfície 3D | 3-D surface | com malha (wireframe) e com faixas coloridas | Planejado | nativo `c:surface3DChart` |
| T89 | Contorno (superfície vista de cima) | Contour | curvas de nível; wireframe | Planejado | nativo `c:surfaceChart` |
| T90 | Dispersão 3D | 3-D scatter | rotação e elevação configuráveis | Planejado | imagem/formas |

### 1.11 Mapas

| Id | Gráfico (pt-BR) | Inglês | Variantes | Estado | OOXML |
|---|---|---|---|---|---|
| T91 | Mapa coroplético | Filled map / choropleth | países, estados e municípios do Brasil; geometria **embutida no pacote**, sem baixar tiles | Planejado | chartex `layoutId="regionMap"` (no Office exige Bing; aqui sai como imagem/formas) |
| T92 | Mapa de bolhas | Bubble map | pontos de latitude/longitude | Planejado | imagem/formas |
| T93 | Cartograma de blocos | Tile grid map | cada região um quadrado | Planejado | imagem/formas |

### 1.12 Minigráficos

| Id | Gráfico (pt-BR) | Inglês | Variantes | Estado | OOXML |
|---|---|---|---|---|---|
| T94 | Minigráfico de linha | Sparkline line | dentro de uma célula, sem eixos | Planejado | `x14:sparklineGroup` (extensão da planilha) |
| T95 | Minigráfico de coluna | Sparkline column | | Planejado | `x14:sparklineGroup` |
| T96 | Minigráfico ganho/perda | Sparkline win/loss | | Planejado | `x14:sparklineGroup` |
| T97 | Minigráfico de barra de progresso e de dados | Data bar | barra dentro da célula | Planejado | formatação condicional (`x14:dataBar`) |

**Contagem do catálogo:** ver `Resumo de contagens`, no fim da seção 2.

#### Observações sobre o que já está pronto

- Os 7 tipos da camada `Grafico` (T01, T04, T20, T24 simples, T30, T36, T48)
  têm eixos com ticks automáticos, linha do zero com negativos, paleta,
  contorno, SVG e PDF.
- Os tipos Parcial existem em `motor::renderizar(TipoGrafico, &Entrada, largura, altura)`
  e geram uma `Cena` válida, mas sem eixos, sem rótulos e sem aparência.
  Os ids herdados (`column-stacked-100`, `step-area-stacked`, `waterfall`…)
  continuam estáveis e serão mantidos como nomes de arquivo.
- Nenhum gráfico 3D (T07-T09, T25 completo, T33, T39, T51, T88, T90) está
  pronto. A projeção do T25 é uma isometria simples, sem profundidade real.

---

## 2. Personalização: 100% das propriedades

A regra é simples: **tudo que aparece no gráfico tem propriedade**. Nenhum
valor de aparência fica dentro do código de um tipo. As propriedades são
agrupadas por elemento do gráfico, e cada elemento é endereçável por caminho
(`grafico.eixo_y.titulo.fonte.tamanho`), o que permite aplicar modelos,
herdar do tema e sobrepor só o necessário.

Regras gerais de todas as propriedades:

- **Herança em três níveis**: padrão do tema, modelo do usuário, valor do
  gráfico. O que não for definido herda do nível de cima.
- **Valores validados** (finitos, dentro de faixas) com erro tipado; nunca
  pânico, nunca NaN na `Cena`.
- **Serializáveis**: cada propriedade tem representação estável para os
  arquivos `.fshz`, `.fdoz` e `.fsdz`.
- **Unidades**: tamanhos de fonte e contornos em pontos; margens podem ser
  relativas (fração da área) ou absolutas.

### 2.1 Área do gráfico e área de plotagem

| Propriedade | Opções | Estado |
|---|---|---|
| Tamanho da cena | largura e altura em pontos, mínimo 40 x 40 | Pronto |
| Margens da área de plotagem (esquerda, direita, superior, inferior) | fração relativa (`LayoutGrafico`), proporcional ao tamanho | Pronto |
| Margens absolutas | pontos, para encaixar em moldura fixa | Planejado |
| Posição e tamanho livres da área de plotagem | retângulo manual (layout manual do Excel) | Planejado |
| Preenchimento da área do gráfico | sem preenchimento, cor sólida, gradiente linear/radial com paradas, imagem (esticar, lado a lado, ajustar), padrão | Planejado |
| Preenchimento da área de plotagem | os mesmos cinco | Planejado |
| Borda da área do gráfico e da plotagem | cor, espessura, tracejado, junção | Planejado |
| Cantos arredondados | raio por canto | Planejado |
| Sombra | cor, deslocamento X/Y, desfoque, transparência | Planejado |
| Brilho e bordas suaves | raio, cor | Planejado |
| Transparência global | 0 a 100% | Pronto (`opacidade` das séries) |
| Fundo transparente na exportação | SVG/PDF/PNG sem fundo | Planejado |
| Proporção bloqueada | manter a razão largura/altura ao redimensionar | Planejado |
| Moldura de página (PDF) | tamanho de papel, margem, orientação | Planejado |

### 2.2 Título e subtítulo

| Propriedade | Opções | Estado |
|---|---|---|
| Texto do título | livre, ou vinculado a uma célula/campo | Pronto (texto) / Planejado (vínculo) |
| Mostrar ou ocultar título | | Pronto |
| Subtítulo | texto livre | Pronto |
| Posição | acima, sobre o gráfico (sobreposto), manual (X/Y), alinhamento esquerda/centro/direita | Planejado |
| Fonte, tamanho, cor, negrito, itálico, sublinhado, espaçamento entre letras | por elemento | Planejado (hoje só um tamanho global) |
| Preenchimento, borda e sombra do título | iguais às da área do gráfico | Planejado |
| Quebra de linha e largura máxima | com reticências | Parcial (reduz com reticências) |
| Rotação | graus | Planejado |

### 2.3 Legenda

| Propriedade | Opções | Estado |
|---|---|---|
| Mostrar ou ocultar | | Pronto |
| Posição | acima, abaixo, esquerda, direita, canto superior direito, sobre a plotagem, manual (X/Y livre) | Planejado |
| Direção | horizontal ou vertical; número de colunas | Planejado |
| Ordem dos itens | série, inversa, personalizada | Planejado |
| Ocultar item | por série ou por ponto (pizza) | Planejado |
| Texto do item | renomear sem mudar a série | Planejado |
| Chave (swatch) | forma, tamanho, espaçamento | Planejado |
| Fonte, cor, tamanho, negrito, itálico | | Planejado |
| Preenchimento, borda, sombra, cantos | | Planejado |
| Legenda interativa | clique alterna a série visível | Planejado |
| Legenda contínua | barra de gradiente para mapa de calor e superfície | Planejado |

### 2.4 Eixos

Eixo X (categorias ou valores), Y (valores), Y secundário, X secundário e Z
(3D). Cada um com todas as propriedades abaixo.

| Propriedade | Opções | Estado |
|---|---|---|
| Ticks numéricos automáticos | escala linear com 5 divisões "bonitas" | Pronto |
| Mínimo, máximo e unidade principal/secundária | automático ou fixo | Planejado |
| Escala | linear, logarítmica (base 2, 10, e, qualquer), raiz quadrada, tempo | Planejado (só linear) |
| Inverter a ordem (valores de cima para baixo) | | Planejado |
| Cruzamento com o outro eixo | automático, em valor, em máximo; categorias entre marcas ou sobre elas | Parcial (linha do zero) |
| Eixo secundário | séries ligadas ao eixo, escala independente | Planejado |
| Formato do número | decimal, moeda, porcentagem; casas; separador de milhar | Pronto (3 formatos, até 6 casas) |
| Formato personalizado | máscara estilo Excel (`#.##0,00`, datas, `[Vermelho]`) | Planejado |
| Unidade de exibição | milhares, milhões, bilhões (rótulo "em milhões") | Parcial (campo `unidade`) |
| Rótulos | mostrar, posição (baixo, alto, ao lado, nenhum), intervalo, saltar | Pronto (mostrar/ocultar) |
| Rótulos girados | ângulo livre, vertical, empilhado | Planejado |
| Quebra e redução de rótulos longos | quebrar, reticências, sobreposição automática | Parcial (reduz) |
| Título do eixo | texto, posição, rotação, fonte | Planejado |
| Linha do eixo | cor, espessura, tracejado, seta, ocultar | Parcial (cor fixa) |
| Marcas (ticks) principais e secundárias | dentro, fora, cruzando, nenhuma; comprimento; espessura | Parcial (marca interna fixa de 4) |
| Quebra de eixo | corte para valores discrepantes, com símbolo | Planejado |
| Eixo de datas | unidade base e principal (dia, mês, ano), formato pt-BR | Planejado |
| Eixo de categorias em níveis | agrupamento com chaves | Planejado |
| Eixo de categorias reverso e ordenação | por valor, por nome | Planejado |
| Eixo polar (ângulo inicial, sentido) | radar, polar, rosca | Planejado |
| Linha de referência | valor fixo, média, mediana, meta, com rótulo e estilo | Planejado |
| Faixa de referência | área sombreada entre dois valores (meta aceitável) | Planejado |

### 2.5 Linhas de grade

| Propriedade | Opções | Estado |
|---|---|---|
| Principais e secundárias (horizontais e verticais) | mostrar/ocultar por eixo | Planejado |
| Cor, espessura, transparência | | Planejado |
| Tracejado | contínuo, pontilhado, tracejado, traço-ponto, padrão livre | Planejado |
| Grade radial e poligonal | radar | Planejado |
| Faixas alternadas | zebra por categoria | Planejado |

### 2.6 Séries

| Propriedade | Opções | Estado |
|---|---|---|
| Nome | livre ou vinculado | Pronto (texto) |
| Cor | cor própria por série | Pronto |
| Paleta cíclica | lista de cores aplicada em ordem | Pronto |
| Transparência / opacidade | por série e global | Pronto (global) |
| Preenchimento | sólido, gradiente (linear, radial, ângulo, paradas), imagem, padrão (hachura, pontos, xadrez), sem preenchimento | Planejado (só sólido) |
| Inverter se negativo | cor diferente abaixo do zero | Planejado |
| Borda | cor, espessura, tracejado, junção, cantos | Pronto (cor e espessura únicas) / Planejado (por série, tracejado) |
| Espessura da linha | pontos | Pronto (global) |
| Tracejado e terminais da linha | contínuo, tracejado, ponto, traço-ponto; arredondado/plano | Planejado |
| Marcadores | forma: círculo, quadrado, losango, triângulo (cima, baixo), cruz, x, estrela, traço, ponto, imagem | Parcial (círculo, quadrado, nenhum) |
| Tamanho do marcador | pontos | Pronto (global) |
| Cor e borda do marcador | preenchimento, contorno, espessura | Parcial |
| Suavização | linha curva por série | Pronto (`Suavizacao`, curva monótona de Fritsch–Carlson em Béziers cúbicas; sem tensão ajustável) |
| Dados ausentes | lacuna, zero, interpolar | Planejado |
| Largura do espaçamento (gap) | % da largura da barra | Planejado (fixo em 72% da faixa) |
| Sobreposição de séries | -100% a 100% | Planejado |
| Larguras máxima e mínima da barra | pontos | Parcial (máximo 112) |
| Ordem de desenho (z-order) | trazer para frente / enviar para trás | Planejado |
| Série no eixo secundário | | Planejado |
| Tipo de gráfico por série | combinado livre (coluna + linha + área) | Planejado |
| Ângulo do primeiro setor e sentido | pizza e rosca | Planejado (fixo em -90 graus) |
| Explosão | de todas as fatias ou de uma só, em % do raio | Planejado |
| Furo da rosca | % do raio | Planejado |
| Série oculta / filtrada | sem apagar os dados | Parcial (`FiltroLegenda`: a cena e os eixos recalculam sem a série; falta lembrar no arquivo) |
| Sombra, brilho, relevo 3D, bisel | | Planejado |
| Profundidade e formato 3D | | Planejado |
| Ordenação visual | por valor, sem alterar a fonte | Planejado |
| Agrupar fatias pequenas ("Outros") | limite em % ou em quantidade | Planejado |

### 2.7 Pontos individuais

| Propriedade | Opções | Estado |
|---|---|---|
| Cor por ponto | cada coluna/fatia/ponto com cor própria | Planejado (a paleta é por série; em pizza é por fatia) |
| Preenchimento, borda e transparência por ponto | | Planejado |
| Rótulo por ponto | texto livre, mostrar/ocultar, posição | Planejado |
| Explosão por fatia | | Planejado |
| Marcador por ponto | forma, tamanho, cor | Planejado |
| Destaque condicional | cor pelo valor (acima da meta, negativo, máximo, mínimo) | Planejado |
| Escala de cor contínua | gradiente de 2 ou 3 cores, divergente, paradas | Planejado (heatmap fixo azul-vermelho) |
| Ponto selecionado | cor de destaque | Pronto (seleção pelo motor de interação) |

### 2.8 Rótulos de dados

| Propriedade | Opções | Estado |
|---|---|---|
| Mostrar valor | | Parcial (`mostrar_valores`) |
| Mostrar nome da categoria / nome da série / porcentagem / tamanho da bolha / chave da legenda | combinação livre | Planejado (pizza tem texto de categoria) |
| Separador | vírgula, ponto e vírgula, nova linha, espaço, texto livre | Planejado |
| Posição | centro, dentro da base, dentro do fim, fora do fim, acima, abaixo, esquerda, direita, melhor ajuste | Planejado |
| Formato do número | decimal, moeda, porcentagem, data, máscara | Pronto (3 formatos) |
| Fonte, cor, tamanho, negrito, itálico | | Planejado |
| Preenchimento, borda, sombra, cantos | caixa do rótulo | Planejado |
| Chamadas (callouts) | forma de balão, linha-guia, ligação entre o rótulo e o ponto | Planejado |
| Rótulo de outra coluna | valores vindos de intervalo, "valor das células" | Planejado |
| Anti-colisão | reposicionar rótulos sobrepostos | Planejado |
| Rotação | graus | Planejado |
| Limite | só máximo, só mínimo, só valores acima de N | Planejado |
| Rótulo do total | soma acima da coluna empilhada | Planejado |

### 2.9 Linhas de tendência e análise

| Propriedade | Opções | Estado |
|---|---|---|
| Tipo | linear, exponencial, logarítmica, polinomial (grau 2 a 6), potência, média móvel (período) | Planejado |
| Previsão | avançar e recuar períodos | Planejado |
| Interseção fixa | forçar passar por um valor | Planejado |
| Equação e R² no gráfico | posição, formato, fonte | Planejado |
| Nome na legenda | | Planejado |
| Estilo da linha | cor, espessura, tracejado | Planejado |
| Banda de confiança | nível, cor, transparência | Planejado |
| Barras de erro | valor fixo, porcentagem, desvio padrão, erro padrão, personalizado (intervalo); mais, menos ou ambos; com ou sem tampa; estilo | Planejado |
| Linha de média, mediana, percentil | por série | Planejado |
| Linhas de queda | da série ao eixo | Planejado |
| Linhas de alta e baixa | entre a maior e a menor série por categoria | Planejado |
| Barras para cima e para baixo | entre a primeira e a última série (abertura-fechamento) | Planejado |
| Linhas de série (conectoras) | entre colunas empilhadas | Planejado |
| Conectores da cascata | linha entre barras | Planejado |

### 2.10 Tabela de dados

| Propriedade | Opções | Estado |
|---|---|---|
| Mostrar tabela de dados sob o gráfico | | Planejado |
| Chaves da legenda na tabela | | Planejado |
| Bordas horizontais, verticais e de contorno | | Planejado |
| Fonte, cor e formato do número | | Planejado |

### 2.11 Anotações e formas sobre o gráfico

| Propriedade | Opções | Estado |
|---|---|---|
| Caixa de texto | posição em coordenadas do gráfico ou dos dados | Planejado |
| Seta e linha | início, fim, ponta, espessura, estilo | Planejado |
| Retângulo, elipse, polígono livre, chave `{` | para destacar regiões | Planejado |
| Imagem (logotipo, marca d'água) | posição, opacidade | Planejado |
| Âncora nos dados | a anotação segue o ponto se os dados mudarem | Planejado |
| Camada acima ou abaixo das séries | | Planejado |
| Caneta/tinta sobre o gráfico | aba Desenhar do Sheet (camada de tinta do `feather-ui`) | Planejado |
| Agrupar e bloquear | | Planejado |

### 2.12 Tipografia

| Propriedade | Opções | Estado |
|---|---|---|
| Tamanho do texto (global) | 4 a 32 | Pronto |
| Família da fonte | por elemento: título, subtítulo, eixos, legenda, rótulos | Planejado (PDF usa fonte base) |
| Tamanho, cor, negrito, itálico, sublinhado, tachado | por elemento | Planejado |
| Espaçamento entre letras e entre linhas | | Planejado |
| Alinhamento | início, centro, fim | Pronto (`AlinhamentoRotulo`) |
| Fonte embutida no PDF/SVG | para o desenho sair igual em qualquer máquina | Planejado |
| Caracteres fora de WinAnsi no PDF | hoje recusados com erro tipado; planejado: fonte Unicode embutida | Parcial |
| Texto confinado à caixa do rótulo | SVG com `textLength` e recorte | Pronto |
| Direção do texto | esquerda-direita, direita-esquerda | Planejado |
| Superscrito e subscrito | unidades como m² | Planejado |

### 2.13 Temas, paletas e modelos

| Propriedade | Opções | Estado |
|---|---|---|
| Paleta própria da série (lista cíclica) | | Pronto |
| Paleta da suíte FeatherWorks | acentos oficiais, claro e escuro | Parcial (cada app injeta a sua: ex. `CORES_SLIDE`) |
| Paletas acessíveis a daltonismo | proteranopia, deuteranopia, tritanopia (Okabe-Ito, Viridis) | Planejado |
| Paletas sequenciais, divergentes e qualitativas | para heatmap e mapa | Planejado |
| Paleta do usuário salva | nome, cores, ordem | Planejado |
| Tema claro, escuro e de impressão | fundo, texto, grade, eixos; segue o `feather-ui` | Planejado |
| Estilos prontos | conjunto de propriedades nomeado (como "Estilo 1" a "Estilo 48" do Excel) | Planejado |
| Modelo de gráfico salvo pelo usuário | exportar/importar todas as propriedades (formato `.fchart`, TOML/JSON) e aplicar a outro gráfico | Planejado |
| Definir como padrão | modelo padrão para novos gráficos | Planejado |
| Herdar tema do documento/slide | cores do tema do Docs e do Slide | Planejado |
| Temáticas de cores | conjuntos nomeados que trocam todas as cores do gráfico de uma vez (fundo, séries, grade, texto): Oceano, Floresta, Pôr do sol, Terra, Neon, Pastel, Outono, Inverno, Café, Monocromático azul/verde/cinza, Corporativo, Alto contraste; escolha com prévia ao passar o mouse; temática do usuário salva | Parcial (`temas`: 14 conjuntos, `Tema::aplicar`; falta prévia ao passar o mouse e temática do usuário salva) |
| Cor por regra | cor do ponto conforme o valor (acima da meta verde, abaixo vermelho; escala de cor contínua) | Planejado |
| Contraste mínimo | aviso quando texto/fundo não atinge 4,5:1 | Planejado |

### 2.14 Animação (FeatherSlide)

| Propriedade | Opções | Estado |
|---|---|---|
| Entrada | surgir, aparecer, varrer, crescer da base, desenhar a linha | Planejado |
| Sequência | tudo junto, por série, por categoria, por ponto | Planejado |
| Duração, atraso e curva (easing) | | Planejado |
| Transição entre dois estados | morfismo de dados antigos para novos | Planejado |
| Quadros determinísticos | `cena_no_instante(t)` para exportar vídeo/GIF e testar | Planejado |

A animação só gera **quadros** (cenas por instante); o relógio fica no
FeatherSlide.

### 2.15 Interação

| Propriedade | Opções | Estado |
|---|---|---|
| Hover (dica ao passar o mouse) | texto da dica, alvo mais próximo, tolerância em unidades da cena | Pronto (`InteracaoGrafico`, `hit_test_dados`, `AlvoGrafico::tooltip`) |
| Clique e seleção | seleciona o ponto, limpa ao clicar fora, Esc | Pronto |
| Editar valor arrastando/digitando | altera a série **com retorno** para a origem | Pronto (`editar_valor`, `editar_selecionado`) |
| Conversão tela para cena | viewport do canvas, sem presumir DPI | Pronto (`ViewportGrafico`) |
| Modelo da dica | campos, formato, HTML-livre não, só texto | Planejado |
| Zoom e rolagem em séries longas | janela de dados, roda do mouse, arrastar, mini-mapa | Planejado |
| Seleção por retângulo (brush) | filtra os pontos | Planejado |
| Cursor cruzado (crosshair) | linhas vertical e horizontal | Planejado |
| Filtro interativo pela legenda | clicar num item da legenda esconde/mostra aquela série ou categoria (pizza: a fatia); eixos e totais se recalculam na hora; Ctrl+clique mostra só aquele item; duplo clique volta tudo; o item filtrado fica esmaecido na legenda | Parcial (motor pronto: `FiltroLegenda`, `clicar_legenda`, `hit_test_legenda`, item esmaecido; o duplo clique e o botão de limpar ficam com o app) |
| Filtro interativo pelos dados | clicar numa barra/fatia/ponto filtra os outros gráficos do mesmo painel por aquela categoria (filtro cruzado) | Planejado |
| Lembrar o filtro | o estado do filtro é guardado com o gráfico no arquivo e pode ser limpo com um botão | Planejado |
| Clique no elemento | evento para o app abrir o diálogo de formatar | Planejado |
| Arrastar elementos | título, legenda, rótulos | Planejado |

### 2.16 Séries grandes (amostragem)

| Propriedade | Opções | Estado |
|---|---|---|
| LTTB (Largest-Triangle-Three-Buckets) | reduz os pontos visuais de linhas, mantém extremos e picos, não altera a série | Pronto (`indices_lttb`, `cena_amostrada`) |
| Limite de pontos | mínimo 3 | Pronto |
| Amostragem em outros tipos | área, dispersão, degraus | Planejado (só linhas) |
| Min-máx por coluna de pixel | envelope para sinais ruidosos | Planejado |
| Agregação feita pela origem | `AmostraDados { disponiveis, usados }` informa o corte | Pronto |
| Desenho progressivo | primeiro quadro rápido, refinamento depois | Planejado |

### 2.17 Idioma e localidade

| Propriedade | Opções | Estado |
|---|---|---|
| Português do Brasil | vírgula decimal, ponto de milhar, `R$` | Pronto (`LocalidadeGrafico::PtBr`) |
| Inglês dos EUA | | Pronto (`EnUs`) |
| Datas pt-BR | `dd/mm/aaaa`, meses e dias por extenso e abreviados | Planejado |
| Outros idiomas | es-ES, pt-PT… | Planejado |
| Porcentagem e moeda por localidade | símbolo, posição, casas | Pronto (R$ e $) |
| Números muito grandes ou pequenos | notação exponencial limitada | Pronto |
| Rejeição de NaN e infinito | erro tipado | Pronto |

### 2.18 Acessibilidade

| Propriedade | Opções | Estado |
|---|---|---|
| Texto alternativo | título e descrição curta para leitores de tela e para `descr` do DrawingML | Planejado |
| Descrição longa gerada | "Colunas: 4 categorias, máximo 20 em B" | Planejado |
| Contraste | verificação e correção automática | Planejado |
| Não depender só da cor | padrões, formas de marcador e rótulos como segunda pista | Planejado |
| Paletas para daltonismo | ver 2.13 | Planejado |
| Navegação por teclado | percorrer pontos com setas, anunciar valor | Planejado |
| Alto contraste | tema próprio | Planejado |
| Respeitar tamanho de fonte do sistema | | Planejado |

### 2.19 Exportação e saída

| Propriedade | Opções | Estado |
|---|---|---|
| Cena vetorial neutra | `Cena` do `feather-draw`, mesma para tela, SVG e PDF | Pronto |
| SVG determinístico | mesma saída, byte a byte | Pronto |
| PDF 1.4 vetorial | formas e texto de rótulo, sem rasterização | Pronto (texto: pizza e tipografia anotada) |
| Resolução e escala | para PNG | Planejado |
| Tamanho do papel e margens | PDF de página | Planejado |
| Camadas no SVG | grupos nomeados (`<g id="serie-1">`) para edição externa | Planejado |
| Metadados | título, autor, descrição no SVG/PDF | Planejado |

### 2.20 Estilos visuais prontos e efeitos

Pedido do usuário (14/10/2026): além de cada propriedade solta, o gráfico tem
que ter **estilos prontos de um clique**, todos editáveis depois, e cada efeito
combinável com qualquer tipo de gráfico em que faça sentido.

| Estilo ou efeito | Como fica | Estado |
|---|---|---|
| Neon | linhas e bordas com brilho (glow) colorido sobre fundo escuro; espessura e intensidade do brilho ajustáveis | Pronto (`AparenciaGrafico::neon`: brilho em linhas, bordas de área e colunas/barras; pizza ainda sem) |
| Barras e colunas arredondadas | raio no topo, nos dois lados ou em todos os cantos; raio em px ou % da largura | Pronto (`Arredondamento`; só topo, topo e base ou todos; empilhadas pelo `motor::renderizar_com`) |
| Linha suavizada | curva (spline monótona, sem passar do valor real) por série, com tensão ajustável | Parcial (curva monótona por série pronta; tensão ajustável pendente) |
| Área com gradiente | preenchimento que esmaece até transparente na base | Pronto (`GradienteArea`; gradiente nativo no SVG e faixas recortadas para o PDF) |
| Gradiente na barra/fatia | linear ou radial por série e por ponto | Planejado |
| Sombra suave | sombra projetada em barras, fatias e linhas | Planejado |
| Vidro (translúcido) | preenchimento semitransparente com borda clara | Planejado |
| Minimalista | sem grade, sem eixos pesados, rótulos direto na série | Planejado |
| Traço à mão (esboço) | contornos irregulares, aspecto de rascunho | Planejado |
| Pictograma | barras feitas de ícones repetidos (ícones do feather-shapes) | Planejado |
| Rosca fina / anel de progresso | espessura do anel, cantos arredondados nas pontas | Planejado |
| Marcadores estilizados | forma, tamanho, contorno, brilho, imagem ou ícone | Planejado |
| Temas | claro, escuro, neon, pastel, corporativo, alto contraste, monocromático, daltônico; tema do usuário salvo | Parcial (14 temáticas de cor em `temas`; faltam daltônico e tema do usuário salvo) |

### 2.21 Vários gráficos juntos

| Recurso | Como fica | Estado |
|---|---|---|
| Combinado no mesmo eixo | cada série com o próprio tipo (coluna + linha + área…) | Parcial (geometria do combinado) |
| Eixo secundário | séries à direita com escala própria | Planejado |
| Sobrepostos | dois gráficos completos um sobre o outro, com transparência | Planejado |
| Painel (dashboard) | vários gráficos numa grade, com título geral e legenda compartilhada | Planejado |
| Quadros pequenos (small multiples) | o mesmo gráfico repetido por categoria, escalas iguais | Planejado |
| Gráfico dentro de gráfico | miniatura de detalhe/ampliação num canto | Planejado |
| Interação sincronizada | cursor, seleção e zoom compartilhados entre os gráficos do painel | Planejado |

#### Resumo de contagens

Contagem feita sobre as tabelas desta página (a linha T24 conta como pronta
pelo degraus simples).

| Item | Prontos | Parciais | Planejados | Total |
|---|---|---|---|---|
| Tipos de gráfico (T01 a T97) | 7 | 17 | 73 | 97 |
| Propriedades de personalização (seção 2) | 42 | 18 | 162 | 222 |


---

## 3. Contrato de dados

A biblioteca **não lê planilha**. O aplicativo entrega séries prontas:

```text
Grafico        titulo, tipo, categorias[], series[]          (camada estável)
Serie          nome, valores[] (f64), cor
Entrada        titulo, categorias[], series[],
               x_values?, z_values?                           (camada do motor)
ContratoGrafico  grafico, metadados_series[], amostra?, avisos[]
MetadadosSerie   indice_origem, nome, cor, agregacao?
AmostraDados     disponiveis, usados
```

Regras:

- Todas as séries têm o mesmo tamanho que `categorias`; valores só finitos.
- Pizza: uma série, sem valores negativos e com pelo menos um positivo.
- Dispersão: as categorias são números válidos (o X). `x_values` dá o X real
  para dispersão, bolhas e Linha 3D; `z_values` dá o tamanho das bolhas ou o
  eixo Z.
- `ContratoGrafico` exige que os metadados batam com as séries (nome e cor) e
  que `usados <= disponiveis`. O índice de origem sobrevive a filtros.
- A agregação (soma, média, contagem) é **sempre da origem**; o campo
  `agregacao` só registra o que foi feito.

Como cada aplicativo entrega os dados:

| Aplicativo | Origem | Vínculo | Estado |
|---|---|---|---|
| FeatherSheet | intervalo de células (`A1:D20`), tabela, tabela dinâmica ou consulta; o Sheet agrega e filtra | referência ao intervalo salva no gráfico; atualiza ao recalcular; mudança só nas células tocadas | Pronto (desenho pelo contrato) / Planejado (vínculo dinâmico completo com todos os tipos) |
| FeatherDocs | tabela do texto: cabeçalho vira séries, primeira coluna vira categorias | SVG embutido no texto com comentário `FeatherOfficeChart:v1` guardando tabela, tipo e título; sobrevive à importação DOCX porque guarda também a ordem da tabela e os cabeçalhos | Pronto (colunas, barras, linhas, área, pizza, dispersão; limites de 5.000 categorias e 16 séries) |
| FeatherSlide | grade (linhas e colunas) do elemento do slide; ou dados editados no FeatherSheet | grade guardada no `.fsdz`; conversão para `Grafico` com paleta do slide | Pronto (via `Grafico` e `cena_motor`) |
| Qualquer um | séries soltas em código (testes, exemplos, MCP) | nenhum | Pronto |

### Editar os dados do gráfico (FeatherDocs e FeatherSlide)

Decisão do usuário (14/10/2026), vale para todo gráfico fora do FeatherSheet:

| Situação | O que o gráfico oferece | Estado |
|---|---|---|
| FeatherSheet **instalado** | "Abrir tabela no FeatherSheet": a tabela que alimenta o gráfico abre no FeatherSheet, com tudo o que ele faz; ao salvar, o gráfico se atualiza | Planejado |
| FeatherSheet **não instalado** | mini janela de dados: interface enxuta, sem muita coisa na tela, mas capaz de fazer bastante coisa com os dados (o quanto ela calcula ainda será definido) | Planejado |

A detecção usa o instalador modular da suíte (cada produto pode ser instalado
sozinho). A biblioteca continua sem ler planilha: as duas janelas só entregam
as séries pelo contrato acima.

Planejado:

- **Séries com X temporal** (`Vec<f64>` de datas serial) e eixo de datas.
- **Matriz** (linhas x colunas) para mapa de calor, superfície e Sankey.
- **Hierarquia** (caminho + valor) para mapa de árvore e explosão solar.
- **Intervalos** (início e fim) para Gantt, faixa, cascata com totais.
- **Valores ausentes** (`Option<f64>`) além de NaN rejeitado.
- **Rótulos vindos de outra coluna** e **cores vindas de outra coluna**.
- **Geometria geográfica embutida** para mapas (sem rede).

---

## 4. Importação e exportação

| Formato | Direção | O que carrega | Estado |
|---|---|---|---|
| **SVG** | exportar | formas + rótulos da pizza com texto confinado; determinístico | Pronto |
| **SVG anotado** | exportar | título, subtítulo, eixos, legenda e valores como texto | Pronto (`exportar_svg_anotado`) |
| **PDF** | exportar | PDF 1.4 vetorial, texto posicionável; sem rasterização | Pronto (`exportar_pdf`, `exportar_pdf_anotado`) |
| **PNG** | exportar | rasterização da `Cena` com resolução configurável | Planejado |
| **Cena** (formas) | exportar | `Cena` do `feather-draw` para telas Iced | Pronto |
| **.xlsx** | importar | gráficos do pacote `xl/charts/chart*.xml`; hoje o FeatherSheet lê só o tipo (coluna, barra, linha, pizza) e as séries, no `feathersheet-io`, fora desta biblioteca | Parcial |
| **.xlsx** | exportar | `c:chartSpace` completo com `c:plotArea`, séries, eixos, formatação | Planejado |
| **.pptx** | importar e exportar | `ppt/charts/chartN.xml` + planilha embutida (`embeddings`) | Planejado |
| **.docx** | importar e exportar | `word/charts/chartN.xml` | Planejado (hoje o Docs guarda SVG embutido) |
| **chartex** (`cx:chart`) | importar e exportar | treemap, sunburst, funil, cascata, histograma, Pareto, caixa e bigode, mapa | Planejado |
| **.ods / .odp / .odt** | importar e exportar | `draw:object` com `chart:chart` (OpenDocument) | Planejado |
| **Modelo `.fchart`** | importar e exportar | todas as propriedades de aparência, sem dados | Planejado |
| **JSON de especificação** | importar e exportar | contrato de dados + aparência, para o MCP e para testes | Planejado |
| **CSV** | exportar | os dados exatamente como desenhados | Planejado |

Quando o tipo não tem equivalente nativo (ver coluna **OOXML** do catálogo),
a exportação grava uma **imagem vetorial** (SVG, ou grupo de formas) e uma
**cópia dos dados** em parte própria do pacote. Assim o arquivo abre no
Office mostrando o desenho certo, e o FeatherWorks continua a poder editá-lo.
A leitura de gráficos do Office desconhecidos nunca descarta o XML original:
guarda a parte intacta e mostra uma imagem de reserva.

---

## 5. Como usar

No `Cargo.toml` do aplicativo (por tag, no estilo do `feather-ui`):

```toml
[dependencies]
feather-charts = { git = "https://github.com/feather-works/feather-charts", tag = "v0.3.1" }
```

Enquanto o crate não for extraído, o nome em uso no monorepo é
`featheroffice-charts`, em `featheroffice/crates/featheroffice-charts`. Para
desenvolver a biblioteca e o aplicativo lado a lado:

```toml
[patch."https://github.com/feather-works/feather-charts"]
feather-charts = { path = "../feather-charts" }
```

Gráfico de colunas com aparência personalizada, SVG e PDF (nomes da API
atual, de `featheroffice-charts`):

```rust
use featheroffice_charts::{
    AparenciaGrafico, Grafico, Marcador, Serie, Tipo,
};
use featheroffice_drawing::Cor;

let grafico = Grafico {
    titulo: "Receita".into(),
    tipo: Tipo::Linhas,
    categorias: vec!["Jan".into(), "Fev".into(), "Mar".into()],
    series: vec![Serie {
        nome: "Vendas".into(),
        valores: vec![10.0, 20.0, 15.0],
        cor: Cor::rgb(11, 87, 208),
    }],
};

let aparencia = AparenciaGrafico {
    paleta: vec![Cor::rgb(43, 122, 120)],   // cor por série, cíclica
    opacidade: 0.9,
    espessura_linha: 2.5,
    marcador: Marcador::Circulo,
    tamanho_marcador: 6.0,
    ..Default::default()
};

let cena = grafico.cena_estilizada(640.0, 360.0, &aparencia).expect("dados válidos"); // Cena para o Iced
let svg  = grafico.exportar_svg(640.0, 360.0).expect("svg");                          // vetorial, determinístico
let pdf  = grafico.exportar_pdf(640.0, 360.0).expect("pdf");                          // PDF 1.4 vetorial
let reduzida = grafico.cena_amostrada(640.0, 360.0, 500).expect("lttb");              // dados intactos
```

Texto, formato regional e subtítulo (camada tipográfica):

```rust
use featheroffice_charts::{FormatoNumero, LocalidadeGrafico, TipografiaGrafico};

let tipografia = TipografiaGrafico {
    subtitulo: "Primeiro trimestre".into(),
    unidade: "R$ mil".into(),
    localidade: LocalidadeGrafico::PtBr,
    formato: FormatoNumero::Moeda(2),
    mostrar_valores: true,
    ..Default::default()
};
let anotada = grafico.cena_anotada(640.0, 360.0, &tipografia).expect("anotada");
// anotada.cena: formas; anotada.rotulos: texto posicionado para o app desenhar
let svg = grafico.exportar_svg_anotado(640.0, 360.0, &tipografia).expect("svg");
```

Tipos adicionais pelo motor de 25 formatos (geometria básica):

```rust
use featheroffice_charts::motor::{renderizar_svg, Entrada, TipoGrafico};

let tipo = TipoGrafico::pelo_id("waterfall").unwrap();
let svg = renderizar_svg(tipo, &entrada, 480.0, 300.0).expect("dados válidos");
```

Interação (hover, clique): `InteracaoGrafico::processar(evento, &grafico, largura, altura, viewport, tolerancia, &aparencia)`
recebe um `EventoGrafico` com o `ViewportGrafico` do canvas e devolve a
`MudancaInteracao`; `AlvoGrafico::tooltip()` monta a dica. A edição de valor é
`Grafico::editar_valor(serie, categoria, novo)`, que revalida o gráfico e
desfaz se o novo valor for inválido.

> As assinaturas acima foram conferidas com o código do crate de origem. A
> API final da `feather-charts` unirá `Grafico`, `AparenciaGrafico`,
> `TipografiaGrafico` e `TipoGrafico` em um só construtor; os nomes podem mudar
> na extração, com aviso no `CHANGELOG`.

## Desenvolvimento e testes

```bash
cargo test
cargo clippy --all-targets
cargo run --example cht018_25_tipos      # gera os 25 tipos do motor
cargo run --example estilos -- /tmp/estilos   # galeria de estilos, temáticas, neon e filtro
```

Regras para contribuir:

- **Todo tipo novo** entra com: um módulo próprio em `motor/tipos/`, teste de
  validação de dados, teste de determinismo (SVG idêntico em duas chamadas),
  teste de que **a série original não muda**, e a linha correspondente na
  tabela do catálogo deste README.
- **Toda propriedade nova** entra com: valor padrão que reproduz o desenho
  atual, validação com erro tipado, serialização e a linha na seção 2. Nada de
  cor, tamanho ou espaçamento fixo dentro de um tipo.
- Só marque Pronto quando existir teste. Estado "parcial" descreve o que falta.
- Sem emoji nem caractere fazendo papel de ícone na saída dos gráficos; texto
  em português do Brasil; commits com o prefixo de quem fez (`claude:`, `gemini:`).
- Sem dependência com biblioteca em C por trás.
- Não afirme que testes Cargo foram executados sem executá-los.

## Licença

MIT (ver `LICENSE`). Paletas acessíveis (Okabe-Ito, Viridis) são de
domínio público ou licença permissiva e terão a atribuição em
`dados/PALETAS-LICENSE.txt` quando entrarem.
