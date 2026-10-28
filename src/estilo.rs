//! Modelo de estilo completo do gráfico ("Formatar gráfico").
//!
//! Tudo é opcional campo a campo: o que fica `None` herda do tema escolhido
//! e, sem tema, da aparência de sempre do motor. O modelo é serializável
//! (JSON) e vai inteiro no arquivo do aplicativo (`.fshz`, `.fdoz`, `.fsdz`).
//! O desenho que respeita este modelo está em [`crate::formatado`].
//!
//! Convenções:
//! - `eixo_x` é o eixo das **categorias** (ou o X numérico da dispersão) e
//!   `eixo_y` o dos **valores**, qualquer que seja a orientação (no gráfico
//!   de barras horizontais o eixo de valores fica embaixo). `eixo_y2` é o
//!   eixo secundário de valores, à direita.
//! - Índices de série e de ponto contam do zero, na ordem do gráfico.
//! - Medidas em unidades do desenho (px do SVG).
use feather_draw::Cor;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::collections::BTreeMap;

// ------------------------------------------------------------------ cor

/// Cor RGBA gravada como texto `#rrggbb` (ou `#rrggbbaa` com transparência).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CorEstilo(pub Cor);

impl CorEstilo {
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self(Cor::rgba(r, g, b, 255))
    }
    pub const fn hex(v: u32) -> Self {
        Self::rgb((v >> 16) as u8, (v >> 8) as u8, v as u8)
    }
    /// `#rgb`, `#rrggbb` ou `#rrggbbaa` (com ou sem `#`).
    pub fn de_texto(s: &str) -> Option<Self> {
        let h = s.trim().trim_start_matches('#');
        let byte = |i: usize| u8::from_str_radix(h.get(i..i + 2)?, 16).ok();
        match h.len() {
            3 => {
                let c = |i: usize| u8::from_str_radix(&h[i..i + 1], 16).ok().map(|v| v * 17);
                Some(Self(Cor::rgba(c(0)?, c(1)?, c(2)?, 255)))
            }
            6 => Some(Self(Cor::rgba(byte(0)?, byte(2)?, byte(4)?, 255))),
            8 => Some(Self(Cor::rgba(byte(0)?, byte(2)?, byte(4)?, byte(6)?))),
            _ => None,
        }
    }
    pub fn texto(&self) -> String {
        let c = self.0;
        if c.a == 255 {
            format!("#{:02x}{:02x}{:02x}", c.r, c.g, c.b)
        } else {
            format!("#{:02x}{:02x}{:02x}{:02x}", c.r, c.g, c.b, c.a)
        }
    }
}
impl From<Cor> for CorEstilo {
    fn from(c: Cor) -> Self {
        Self(c)
    }
}
impl Serialize for CorEstilo {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.texto())
    }
}
impl<'de> Deserialize<'de> for CorEstilo {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        Self::de_texto(&s).ok_or_else(|| serde::de::Error::custom("cor inválida"))
    }
}

// ------------------------------------------------------------- peças comuns

/// Padrão do traço de uma linha (borda, eixo, grade, série).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Traco {
    #[default]
    Continuo,
    Tracejado,
    Pontilhado,
    TracoPonto,
}
impl Traco {
    pub const TODOS: [Self; 4] = [Self::Continuo, Self::Tracejado, Self::Pontilhado, Self::TracoPonto];
    pub fn nome(self) -> &'static str {
        match self {
            Self::Continuo => "Contínuo",
            Self::Tracejado => "Tracejado",
            Self::Pontilhado => "Pontilhado",
            Self::TracoPonto => "Traço e ponto",
        }
    }
    /// `stroke-dasharray` proporcional à espessura (vazio = contínuo).
    pub fn dasharray(self, espessura: f32) -> Option<String> {
        let l = espessura.max(1.0);
        let v: &[f32] = match self {
            Self::Continuo => return None,
            Self::Tracejado => &[4.0, 3.0],
            Self::Pontilhado => &[1.0, 2.0],
            Self::TracoPonto => &[4.0, 2.0, 1.0, 2.0],
        };
        Some(v.iter().map(|x| format!("{:.1}", x * l)).collect::<Vec<_>>().join(" "))
    }
}

/// Preenchimento de uma área.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "tipo", rename_all = "snake_case")]
pub enum Preenchimento {
    Nenhum,
    Solido { cor: CorEstilo },
    /// Degradê linear; `angulo` em graus (0 = esquerda→direita, 90 = cima→baixo).
    Degrade { inicio: CorEstilo, fim: CorEstilo, #[serde(default)] angulo: f32 },
}

/// Linha (borda, eixo, grade). `None` herda.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Linha {
    pub visivel: Option<bool>,
    pub cor: Option<CorEstilo>,
    pub espessura: Option<f32>,
    pub traco: Option<Traco>,
}

/// Sombra projetada.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Sombra {
    pub cor: CorEstilo,
    pub desfoque: f32,
    pub dx: f32,
    pub dy: f32,
}
impl Default for Sombra {
    fn default() -> Self {
        Self { cor: CorEstilo(Cor::rgba(0, 0, 0, 70)), desfoque: 4.0, dx: 2.0, dy: 3.0 }
    }
}

/// Área do gráfico ou de plotagem: preenchimento, borda, sombra e cantos.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Area {
    pub preenchimento: Option<Preenchimento>,
    pub borda: Linha,
    pub sombra: Option<Sombra>,
    /// Raio dos cantos.
    pub raio: Option<f32>,
}

/// Margens internas em px. `None` = automático.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Margens {
    pub superior: Option<f32>,
    pub inferior: Option<f32>,
    pub esquerda: Option<f32>,
    pub direita: Option<f32>,
}

/// Fonte de um texto. `None` herda da fonte geral do gráfico.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Texto {
    pub familia: Option<String>,
    pub tamanho: Option<f32>,
    pub cor: Option<CorEstilo>,
    pub negrito: Option<bool>,
    pub italico: Option<bool>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Alinhamento {
    Esquerda,
    #[default]
    Centro,
    Direita,
}

// ------------------------------------------------------------ título/legenda

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Titulo {
    pub visivel: Option<bool>,
    /// Texto próprio; `None` usa o título do gráfico.
    pub texto: Option<String>,
    pub fonte: Texto,
    pub alinhamento: Option<Alinhamento>,
    /// Sobre a área de plotagem, sem reservar espaço em cima.
    pub sobreposto: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Subtitulo {
    pub texto: Option<String>,
    pub fonte: Texto,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PosicaoLegenda {
    #[default]
    Inferior,
    Superior,
    Esquerda,
    Direita,
    Nenhuma,
}
impl PosicaoLegenda {
    pub const TODAS: [Self; 5] = [Self::Inferior, Self::Superior, Self::Esquerda, Self::Direita, Self::Nenhuma];
    pub fn nome(self) -> &'static str {
        match self {
            Self::Inferior => "Embaixo",
            Self::Superior => "Em cima",
            Self::Esquerda => "À esquerda",
            Self::Direita => "À direita",
            Self::Nenhuma => "Sem legenda",
        }
    }
}

/// Um item da legenda (uma série; na pizza, uma categoria).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ItemLegenda {
    pub texto: Option<String>,
    /// Cor da amostra na legenda; `None` = a cor da série/fatia.
    pub cor: Option<CorEstilo>,
    pub oculto: bool,
    /// Desligado pelo clique na legenda: o item continua na legenda (amostra
    /// hachurada), mas a série (na pizza, a fatia) some do desenho.
    pub desligado: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Legenda {
    pub posicao: Option<PosicaoLegenda>,
    pub fonte: Texto,
    pub itens: BTreeMap<usize, ItemLegenda>,
    /// Ordem dos itens (índices); os que faltarem vêm depois, na ordem natural.
    pub ordem: Vec<usize>,
}
impl Legenda {
    /// Índices na ordem de exibição, sem os ocultos.
    pub fn ordem_visivel(&self, total: usize) -> Vec<usize> {
        let mut saida = self.ordem_completa(total);
        saida.retain(|i| !self.itens.get(i).is_some_and(|it| it.oculto));
        saida
    }
    /// A série (na pizza, a fatia) `i` foi desligada pelo clique na legenda.
    pub fn desligado(&self, i: usize) -> bool {
        self.itens.get(&i).is_some_and(|it| it.desligado)
    }
    /// Liga/desliga o item `i` (clique na legenda).
    pub fn alternar(&mut self, i: usize) {
        let it = self.itens.entry(i).or_default();
        it.desligado = !it.desligado;
    }
    /// Ordem completa (com ocultos), para a interface reordenar.
    pub fn ordem_completa(&self, total: usize) -> Vec<usize> {
        let mut saida: Vec<usize> = Vec::with_capacity(total);
        for i in self.ordem.iter().copied().chain(0..total) {
            if i < total && !saida.contains(&i) {
                saida.push(i);
            }
        }
        saida
    }
}

// ------------------------------------------------------------------- eixos

/// Formato dos números (eixo ou rótulos de dados).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(tag = "tipo", rename_all = "snake_case")]
pub enum FormatoNum {
    /// Casas decimais pelo próprio número.
    #[default]
    Automatico,
    Decimal { casas: u8 },
    Moeda { casas: u8 },
    Percentual { casas: u8 },
}
impl FormatoNum {
    pub fn nome(self) -> &'static str {
        match self {
            Self::Automatico => "Automático",
            Self::Decimal { .. } => "Número",
            Self::Moeda { .. } => "Moeda (R$)",
            Self::Percentual { .. } => "Porcentagem",
        }
    }
    pub fn casas(self) -> u8 {
        match self {
            Self::Automatico => 0,
            Self::Decimal { casas } | Self::Moeda { casas } | Self::Percentual { casas } => casas,
        }
    }
    pub fn com_casas(self, casas: u8) -> Self {
        let casas = casas.min(6);
        match self {
            Self::Automatico => Self::Automatico,
            Self::Decimal { .. } => Self::Decimal { casas },
            Self::Moeda { .. } => Self::Moeda { casas },
            Self::Percentual { .. } => Self::Percentual { casas },
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Eixo {
    pub visivel: Option<bool>,
    pub minimo: Option<f64>,
    pub maximo: Option<f64>,
    /// Distância entre marcas principais (rótulos e grade principal).
    pub unidade_principal: Option<f64>,
    /// Distância entre marcas secundárias (grade secundária).
    pub unidade_secundaria: Option<f64>,
    /// Escala logarítmica de base 10 (só valores positivos).
    pub logaritmica: bool,
    /// Valores (ou categorias) em ordem inversa.
    pub invertido: bool,
    /// Onde o outro eixo cruza este: um valor (eixo de valores) ou o número
    /// da categoria, contando de 1 (eixo de categorias).
    pub cruzamento: Option<f64>,
    pub formato: Option<FormatoNum>,
    pub rotulos: Texto,
    /// Giro dos rótulos em graus (−90 a 90).
    pub rotacao: Option<f32>,
    pub titulo: Option<String>,
    pub fonte_titulo: Texto,
    /// Texto da unidade (R$, kg) no alto do eixo de valores.
    pub unidade: Option<String>,
    pub linha: Linha,
    pub grade_principal: Linha,
    pub grade_secundaria: Linha,
}

// ------------------------------------------------------------ séries/pontos

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FormaMarcador {
    #[default]
    Nenhum,
    Circulo,
    Quadrado,
    Losango,
    Triangulo,
    Cruz,
}
impl FormaMarcador {
    pub const TODAS: [Self; 6] = [Self::Nenhum, Self::Circulo, Self::Quadrado, Self::Losango, Self::Triangulo, Self::Cruz];
    pub fn nome(self) -> &'static str {
        match self {
            Self::Nenhum => "Sem marcador",
            Self::Circulo => "Círculo",
            Self::Quadrado => "Quadrado",
            Self::Losango => "Losango",
            Self::Triangulo => "Triângulo",
            Self::Cruz => "Cruz",
        }
    }
}

/// Estilo próprio de um ponto (coluna, barra, fatia, marcador ou trecho da
/// linha que chega nele): vale mais que o da série.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct EstiloPonto {
    pub cor: Option<CorEstilo>,
    pub contorno: Option<CorEstilo>,
    /// Pizza: afasta a fatia do centro (px).
    pub destacar: Option<f32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PosicaoRotulo {
    #[default]
    Automatica,
    Centro,
    DentroFim,
    DentroBase,
    ForaFim,
    Acima,
    Abaixo,
    Esquerda,
    Direita,
}
impl PosicaoRotulo {
    pub const TODAS: [Self; 9] = [
        Self::Automatica, Self::Centro, Self::DentroFim, Self::DentroBase, Self::ForaFim,
        Self::Acima, Self::Abaixo, Self::Esquerda, Self::Direita,
    ];
    pub fn nome(self) -> &'static str {
        match self {
            Self::Automatica => "Automática",
            Self::Centro => "Centro",
            Self::DentroFim => "Dentro, no fim",
            Self::DentroBase => "Dentro, na base",
            Self::ForaFim => "Fora, no fim",
            Self::Acima => "Acima",
            Self::Abaixo => "Abaixo",
            Self::Esquerda => "À esquerda",
            Self::Direita => "À direita",
        }
    }
}

/// Rótulos de dados (globais ou por série).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Rotulos {
    pub visivel: Option<bool>,
    pub posicao: Option<PosicaoRotulo>,
    pub formato: Option<FormatoNum>,
    pub fonte: Texto,
    pub valor: Option<bool>,
    pub nome_serie: Option<bool>,
    pub categoria: Option<bool>,
    /// Pizza: porcentagem do total.
    pub percentual: Option<bool>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct EstiloSerie {
    pub cor: Option<CorEstilo>,
    pub contorno: Option<CorEstilo>,
    pub espessura_contorno: Option<f32>,
    /// Espessura da linha (linhas, áreas, degraus).
    pub espessura_linha: Option<f32>,
    pub traco: Option<Traco>,
    pub marcador: Option<FormaMarcador>,
    pub tamanho_marcador: Option<f32>,
    pub suavizar: Option<bool>,
    pub eixo_secundario: bool,
    /// 0 a 1.
    pub opacidade: Option<f32>,
    pub rotulos: Rotulos,
    pub pontos: BTreeMap<usize, EstiloPonto>,
    /// Tendência, barras de erro e linhas de máximo/mínimo/média.
    #[serde(skip_serializing_if = "crate::analitico::Analise::vazia")]
    pub analise: crate::analitico::Analise,
}

/// Espaçamento das colunas/barras, como no Excel.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Barras {
    /// Espaço entre grupos, em % da largura de uma barra (0 a 500).
    pub espacamento: Option<f32>,
    /// Sobreposição das barras de um grupo, em % (−100 separa, 100 empilha por cima).
    pub sobreposicao: Option<f32>,
    /// Raio dos cantos das barras.
    pub raio: Option<f32>,
}

/// Colunas/barras lado a lado, empilhadas ou empilhadas em 100%.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Empilhar {
    #[default]
    Nenhum,
    Empilhado,
    Cem,
}
impl Empilhar {
    pub fn nenhum(&self) -> bool {
        *self == Self::Nenhum
    }
}

/// A dica que aparece com o mouse sobre um ponto (no aplicativo; não vai
/// no SVG exportado).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Dica {
    /// `Some(false)` desliga a dica.
    pub visivel: Option<bool>,
    /// Título com a categoria (padrão: sim).
    pub categoria: Option<bool>,
    /// Nome da série antes do valor (padrão: sim).
    pub serie: Option<bool>,
    /// Porcentagem do total da categoria (ou da pizza).
    pub percentual: bool,
    /// `None` = o formato dos rótulos/eixo.
    pub formato: Option<FormatoNum>,
    pub prefixo: String,
    pub sufixo: String,
    pub fundo: Option<CorEstilo>,
    pub texto: Option<CorEstilo>,
    pub tamanho: Option<f32>,
}
impl Dica {
    pub fn vazia(&self) -> bool {
        *self == Self::default()
    }
}

// ---------------------------------------------------------------- o estilo

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct EstiloGrafico {
    /// Identificador de um tema pronto ([`temas`]); `None` = aparência padrão.
    pub tema: Option<String>,
    pub area_grafico: Area,
    pub area_plotagem: Area,
    /// Espaço entre a borda do gráfico e o conteúdo.
    pub margens_grafico: Margens,
    /// Espaço entre a borda da área de plotagem e os dados.
    pub margens_plotagem: Margens,
    /// Fonte geral (herdada por todos os textos).
    pub fonte: Texto,
    pub titulo: Titulo,
    pub subtitulo: Subtitulo,
    pub legenda: Legenda,
    pub eixo_x: Eixo,
    pub eixo_y: Eixo,
    pub eixo_y2: Eixo,
    pub series: BTreeMap<usize, EstiloSerie>,
    /// Rótulos de dados de todas as séries (a série pode sobrepor).
    pub rotulos: Rotulos,
    pub barras: Barras,
    /// Colunas e barras empilhadas (o tipo "Colunas empilhadas" do painel).
    #[serde(skip_serializing_if = "Empilhar::nenhum")]
    pub empilhar: Empilhar,
    /// Linhas e áreas em degraus (o tipo "Área em degraus" do painel). Vem
    /// do tipo do gráfico na hora de desenhar; não é gravado.
    #[serde(skip)]
    pub degraus: bool,
    /// Rosca: fração do raio ocupada pelo furo (o `holeSize` do Excel ÷ 100,
    /// de 0,1 a 0,9). `None` = pizza cheia. Na rosca cada série é um anel,
    /// a primeira por dentro, como no Excel.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rosca: Option<f32>,
    /// Descrição para leitores de tela (vai no `<title>`/`<desc>` do SVG).
    pub texto_alternativo: String,
    /// Dica do mouse.
    #[serde(skip_serializing_if = "Dica::vazia")]
    pub dica: Dica,
}

impl EstiloGrafico {
    pub fn tema(&self) -> Option<&'static TemaGrafico> {
        self.tema.as_deref().and_then(TemaGrafico::pelo_id)
    }
    pub fn serie(&self, i: usize) -> Option<&EstiloSerie> {
        self.series.get(&i)
    }
    pub fn serie_mut(&mut self, i: usize) -> &mut EstiloSerie {
        self.series.entry(i).or_default()
    }
    /// Aplica um tema como no Excel: troca o conjunto e solta as cores
    /// escolhidas à mão nas séries, pontos e áreas (o resto fica).
    pub fn aplicar_tema(&mut self, id: Option<&str>) {
        self.tema = id.filter(|i| TemaGrafico::pelo_id(i).is_some()).map(str::to_owned);
        for s in self.series.values_mut() {
            s.cor = None;
            for p in s.pontos.values_mut() {
                p.cor = None;
            }
        }
        self.area_grafico.preenchimento = None;
        self.area_plotagem.preenchimento = None;
        self.fonte.cor = None;
    }
    /// Nada fora da faixa aceita (números finitos, tamanhos positivos).
    pub fn valido(&self) -> bool {
        let pos = |v: Option<f32>| v.map_or(true, |x| x.is_finite() && x >= 0.0);
        let fin = |v: Option<f64>| v.map_or(true, f64::is_finite);
        let eixo_ok = |e: &Eixo| {
            fin(e.minimo) && fin(e.maximo) && fin(e.cruzamento)
                && e.unidade_principal.map_or(true, |u| u.is_finite() && u > 0.0)
                && e.unidade_secundaria.map_or(true, |u| u.is_finite() && u > 0.0)
                && !(e.minimo.zip(e.maximo).is_some_and(|(a, b)| a >= b))
        };
        let m = |m: &Margens| pos(m.superior) && pos(m.inferior) && pos(m.esquerda) && pos(m.direita);
        eixo_ok(&self.eixo_x) && eixo_ok(&self.eixo_y) && eixo_ok(&self.eixo_y2)
            && m(&self.margens_grafico) && m(&self.margens_plotagem)
            && self.rosca.map_or(true, |f| f.is_finite() && (0.0..1.0).contains(&f))
            && self.series.values().all(|s| pos(s.espessura_linha) && pos(s.espessura_contorno)
                && pos(s.tamanho_marcador) && s.opacidade.map_or(true, |o| (0.0..=1.0).contains(&o)))
    }
}

// ------------------------------------------------------------------- temas

/// Tema pronto: cores, fonte e estilo aplicados com um clique.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TemaGrafico {
    pub id: &'static str,
    pub nome: &'static str,
    pub fundo: CorEstilo,
    /// Fundo da área de plotagem (`None` = transparente).
    pub plotagem: Option<CorEstilo>,
    pub texto: CorEstilo,
    pub eixo: CorEstilo,
    pub grade: CorEstilo,
    pub grade_visivel: bool,
    pub paleta: &'static [CorEstilo],
    pub familia: &'static str,
    /// Borda do gráfico (`None` = sem borda).
    pub borda: Option<CorEstilo>,
    pub raio: f32,
    pub raio_barras: f32,
    pub sombra: bool,
}

const fn h(v: u32) -> CorEstilo {
    CorEstilo::hex(v)
}
const SANS: &str = "Noto Sans, DejaVu Sans, Liberation Sans, Arial, sans-serif";
const SERIFA: &str = "Noto Serif, DejaVu Serif, Liberation Serif, Georgia, serif";

static TEMAS: [TemaGrafico; 10] = [
    TemaGrafico { id: "padrao", nome: "Padrão", fundo: h(0xFFFFFF), plotagem: None, texto: h(0x202020),
        eixo: h(0x5F6368), grade: h(0xE3E5E8), grade_visivel: true, familia: SANS, borda: None, raio: 0.0,
        raio_barras: 0.0, sombra: false,
        paleta: &[h(0x4285F4), h(0xEA4335), h(0xFBBC04), h(0x34A853), h(0xFF6D01), h(0x46BDC6), h(0x7B61FF)] },
    TemaGrafico { id: "office", nome: "Office", fundo: h(0xFFFFFF), plotagem: None, texto: h(0x595959),
        eixo: h(0xBFBFBF), grade: h(0xD9D9D9), grade_visivel: true, familia: SANS, borda: Some(h(0xD9D9D9)),
        raio: 0.0, raio_barras: 0.0, sombra: false,
        paleta: &[h(0x4472C4), h(0xED7D31), h(0xA5A5A5), h(0xFFC000), h(0x5B9BD5), h(0x70AD47), h(0x264478)] },
    TemaGrafico { id: "oceano", nome: "Oceano", fundo: h(0xF2F8FB), plotagem: None, texto: h(0x12303F),
        eixo: h(0x5E7C8A), grade: h(0xD3E3EA), grade_visivel: true, familia: SANS, borda: None, raio: 10.0,
        raio_barras: 3.0, sombra: false,
        paleta: &[h(0x0B6E99), h(0x2AA9C8), h(0x0F4C75), h(0x6CC5D6), h(0x1B9AAA), h(0x3A7CA5), h(0x8FD3E0)] },
    TemaGrafico { id: "floresta", nome: "Floresta", fundo: h(0xF4F8F2), plotagem: None, texto: h(0x1B3320),
        eixo: h(0x607A5E), grade: h(0xD8E5D3), grade_visivel: true, familia: SANS, borda: None, raio: 10.0,
        raio_barras: 3.0, sombra: false,
        paleta: &[h(0x2D6A4F), h(0x52B788), h(0x1B4332), h(0x95D5B2), h(0x74A057), h(0x40916C), h(0xB7C68B)] },
    TemaGrafico { id: "por-do-sol", nome: "Pôr do sol", fundo: h(0xFFF7F0), plotagem: None, texto: h(0x3B1F2B),
        eixo: h(0x8C6A6A), grade: h(0xF2DACB), grade_visivel: true, familia: SANS, borda: None, raio: 10.0,
        raio_barras: 3.0, sombra: false,
        paleta: &[h(0xE63946), h(0xF77F00), h(0xFCBF49), h(0xD62828), h(0xFF9E7A), h(0x9D4EDD), h(0xF4A261)] },
    TemaGrafico { id: "pastel", nome: "Pastel", fundo: h(0xFFFFFF), plotagem: None, texto: h(0x44444F),
        eixo: h(0x9A9AA8), grade: h(0xECECF2), grade_visivel: true, familia: SANS, borda: None, raio: 12.0,
        raio_barras: 5.0, sombra: false,
        paleta: &[h(0x8EC5E8), h(0xFFA8B8), h(0x9EDFC4), h(0xFFCFA8), h(0xB9C2EE), h(0xD6EBB0), h(0xEAB6E8)] },
    TemaGrafico { id: "corporativo", nome: "Corporativo", fundo: h(0xFFFFFF), plotagem: Some(h(0xF8FAFC)),
        texto: h(0x1F2937), eixo: h(0x6B7280), grade: h(0xE5E7EB), grade_visivel: true, familia: SANS,
        borda: Some(h(0xE5E7EB)), raio: 6.0, raio_barras: 0.0, sombra: true,
        paleta: &[h(0x1D4ED8), h(0x0F766E), h(0xB45309), h(0x7C3AED), h(0xBE123C), h(0x475569), h(0x0891B2)] },
    TemaGrafico { id: "escuro", nome: "Escuro", fundo: h(0x1E2228), plotagem: None, texto: h(0xE8EAED),
        eixo: h(0x8A919B), grade: h(0x353B44), grade_visivel: true, familia: SANS, borda: None, raio: 10.0,
        raio_barras: 2.0, sombra: false,
        paleta: &[h(0x5EA0FF), h(0xFF7A6B), h(0xFFD15C), h(0x4FD18B), h(0xC38BFF), h(0x48D1D8), h(0xFF9F43)] },
    TemaGrafico { id: "monocromatico", nome: "Monocromático", fundo: h(0xFAFAFA), plotagem: None, texto: h(0x1F1F1F),
        eixo: h(0x757575), grade: h(0xE3E3E3), grade_visivel: true, familia: SERIFA, borda: Some(h(0xBDBDBD)),
        raio: 0.0, raio_barras: 0.0, sombra: false,
        paleta: &[h(0x2B2B2B), h(0x606060), h(0x8C8C8C), h(0xB0B0B0), h(0x474747), h(0xCFCFCF), h(0x111111)] },
    TemaGrafico { id: "alto-contraste", nome: "Alto contraste", fundo: h(0x000000), plotagem: None, texto: h(0xFFFFFF),
        eixo: h(0xFFFFFF), grade: h(0x595959), grade_visivel: true, familia: SANS, borda: Some(h(0xFFFFFF)),
        raio: 0.0, raio_barras: 0.0, sombra: false,
        paleta: &[h(0xFFFF00), h(0x00FFFF), h(0xFF00FF), h(0x00FF00), h(0xFF8000), h(0xFFFFFF), h(0xFF4040)] },
];

/// Os temas prontos, na ordem de exibição.
pub fn temas() -> &'static [TemaGrafico] {
    &TEMAS
}

impl TemaGrafico {
    pub fn pelo_id(id: &str) -> Option<&'static TemaGrafico> {
        TEMAS.iter().find(|t| t.id == id)
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn padrao_vazio_ida_e_volta_e_json_antigo() {
        let e: EstiloGrafico = serde_json::from_str("{}").unwrap();
        assert_eq!(e, EstiloGrafico::default());
        let mut e = EstiloGrafico::default();
        e.tema = Some("oceano".into());
        e.area_grafico.preenchimento = Some(Preenchimento::Degrade {
            inicio: CorEstilo::hex(0x112233), fim: CorEstilo(Cor::rgba(1, 2, 3, 128)), angulo: 90.0,
        });
        e.margens_plotagem.esquerda = Some(12.0);
        e.serie_mut(1).pontos.insert(2, EstiloPonto { cor: Some(CorEstilo::hex(0xff0000)), ..Default::default() });
        e.legenda.itens.insert(0, ItemLegenda { oculto: true, ..Default::default() });
        e.eixo_y.formato = Some(FormatoNum::Moeda { casas: 2 });
        e.eixo_y.logaritmica = true;
        let json = serde_json::to_string(&e).unwrap();
        assert!(json.contains("\"#112233\"") && json.contains("\"#01020380\""), "{json}");
        let volta: EstiloGrafico = serde_json::from_str(&json).unwrap();
        assert_eq!(volta, e);
        // Campo desconhecido (versão futura) é ignorado; cor inválida, não.
        assert!(serde_json::from_str::<EstiloGrafico>(r#"{"futuro":1,"tema":"neon"}"#).is_ok());
        assert!(serde_json::from_str::<EstiloGrafico>(r#"{"fonte":{"cor":"azul"}}"#).is_err());
    }

    #[test]
    fn dez_temas_distintos_e_aplicar_solta_cores_manuais() {
        let ids: std::collections::HashSet<_> = temas().iter().map(|t| t.id).collect();
        assert_eq!(ids.len(), 10);
        assert!(temas().iter().all(|t| t.paleta.len() >= 6));
        let mut e = EstiloGrafico::default();
        e.serie_mut(0).cor = Some(CorEstilo::hex(0x123456));
        e.serie_mut(0).espessura_linha = Some(3.0);
        e.aplicar_tema(Some("escuro"));
        assert_eq!(e.tema.as_deref(), Some("escuro"));
        assert_eq!(e.series[&0].cor, None);
        assert_eq!(e.series[&0].espessura_linha, Some(3.0));
        e.aplicar_tema(Some("nao-existe"));
        assert_eq!(e.tema, None);
    }

    #[test]
    fn legenda_ordem_e_ocultos_e_validacao() {
        let mut l = Legenda { ordem: vec![2, 0, 9], ..Default::default() };
        l.itens.insert(0, ItemLegenda { oculto: true, ..Default::default() });
        assert_eq!(l.ordem_visivel(3), vec![2, 1]);
        assert_eq!(l.ordem_completa(3), vec![2, 0, 1]);
        let mut e = EstiloGrafico::default();
        assert!(e.valido());
        e.eixo_y.minimo = Some(10.0);
        e.eixo_y.maximo = Some(5.0);
        assert!(!e.valido());
        e.eixo_y.maximo = Some(50.0);
        e.serie_mut(0).opacidade = Some(2.0);
        assert!(!e.valido());
    }

    #[test]
    fn cor_de_texto_e_traco() {
        assert_eq!(CorEstilo::de_texto("#abc").unwrap().texto(), "#aabbcc");
        assert_eq!(CorEstilo::de_texto("4285F4").unwrap().texto(), "#4285f4");
        assert!(CorEstilo::de_texto("#12").is_none());
        assert_eq!(Traco::Continuo.dasharray(2.0), None);
        assert_eq!(Traco::Tracejado.dasharray(2.0).as_deref(), Some("8.0 6.0"));
    }
}
