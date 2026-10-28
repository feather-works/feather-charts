//! Desenho SVG que respeita o [`EstiloGrafico`] inteiro: áreas (degradê,
//! borda, sombra, cantos), margens, título/subtítulo, legenda por item,
//! eixos (mín/máx, unidades, log, invertido, cruzamento, formato, rotação,
//! título, linha), grades principal/secundária, séries (cor, contorno,
//! espessura, traço, marcador, suavizar, eixo secundário), cor por ponto,
//! rótulos de dados, espaçamento/sobreposição das barras e temas.
//!
//! Serve aos sete tipos básicos (colunas, barras, linhas, área, degraus,
//! pizza, dispersão). Sem estilo (`EstiloGrafico::default()`), o desenho
//! segue a aparência de sempre: fundo branco, texto escuro, sem grade.
use crate::estilo::*;
use crate::tipografia::{FormatoNumero, LocalidadeGrafico};
use crate::{ErroGrafico, Grafico, Tipo};
use feather_draw::Cor;
use std::fmt::Write;

mod avancado;
pub use avancado::{DadosDica, PedidoAvancado, TipoAvancado};

const FAMILIA_PADRAO: &str = "Noto Sans, DejaVu Sans, Liberation Sans, Arial, sans-serif";
const TEXTO_PADRAO: Cor = Cor::rgba(32, 32, 32, 255);
const EIXO_PADRAO: Cor = Cor::rgba(95, 99, 104, 255);
const GRADE_PADRAO: Cor = Cor::rgba(227, 229, 232, 255);
const PIZZA_PADRAO: [Cor; 5] = [
    Cor::rgba(43, 122, 120, 255),
    Cor::rgba(11, 87, 208, 255),
    Cor::rgba(232, 113, 10, 255),
    Cor::rgba(24, 128, 56, 255),
    Cor::rgba(142, 36, 170, 255),
];

// ------------------------------------------------------------ utilidades

fn hex6(c: Cor) -> String {
    format!("#{:02x}{:02x}{:02x}", c.r, c.g, c.b)
}
fn alfa(c: Cor) -> f32 {
    f32::from(c.a) / 255.0
}
fn com_alfa(c: Cor, op: f32) -> Cor {
    Cor::rgba(c.r, c.g, c.b, (f32::from(c.a) * op.clamp(0.0, 1.0)).round() as u8)
}
fn preencher(c: Cor) -> String {
    if c.a == 255 {
        format!("fill=\"{}\"", hex6(c))
    } else {
        format!("fill=\"{}\" fill-opacity=\"{:.3}\"", hex6(c), alfa(c))
    }
}
fn tracar(c: Cor, espessura: f32, traco: Traco) -> String {
    let mut s = format!("stroke=\"{}\" stroke-width=\"{:.2}\"", hex6(c), espessura);
    if c.a != 255 {
        let _ = write!(s, " stroke-opacity=\"{:.3}\"", alfa(c));
    }
    if let Some(d) = traco.dasharray(espessura) {
        let _ = write!(s, " stroke-dasharray=\"{d}\"");
    }
    s
}
fn escapar(t: &str) -> String {
    let mut out = String::with_capacity(t.len());
    for c in t.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            c if c.is_control() => out.push(' '),
            _ => out.push(c),
        }
    }
    out
}
fn largura_texto(t: &str, tamanho: f32) -> f32 {
    t.chars().count() as f32 * tamanho * 0.56
}
fn cortar(t: &str, largura: f32, tamanho: f32) -> String {
    let max = if largura <= 0.0 { 0 } else { (largura / (tamanho * 0.56)).floor() as usize };
    let n = t.chars().count();
    if n <= max {
        return t.to_owned();
    }
    if max < 2 {
        return String::new();
    }
    let mut s: String = t.chars().take(max - 1).collect();
    s.push('…');
    s
}

#[derive(Debug, Clone, Copy)]
struct Ret {
    x: f32,
    y: f32,
    w: f32,
    h: f32,
}
impl Ret {
    fn encolher(self, sup: f32, inf: f32, esq: f32, dir: f32) -> Ret {
        let w = (self.w - esq - dir).max(4.0);
        let h = (self.h - sup - inf).max(4.0);
        Ret { x: self.x + esq.min(self.w - 4.0).max(0.0), y: self.y + sup.min(self.h - 4.0).max(0.0), w, h }
    }
}

/// Fonte já resolvida (herança aplicada).
#[derive(Debug, Clone)]
struct Fonte {
    familia: String,
    tamanho: f32,
    cor: Cor,
    negrito: bool,
    italico: bool,
}
impl Fonte {
    fn de(base: &Fonte, t: &Texto, fator: f32) -> Fonte {
        Fonte {
            familia: t.familia.clone().filter(|f| !f.trim().is_empty()).unwrap_or_else(|| base.familia.clone()),
            tamanho: t.tamanho.filter(|v| v.is_finite() && *v > 0.0).map_or(base.tamanho * fator, |v| v.clamp(4.0, 96.0)),
            cor: t.cor.map_or(base.cor, |c| c.0),
            negrito: t.negrito.unwrap_or(base.negrito),
            italico: t.italico.unwrap_or(base.italico),
        }
    }
    fn atributos(&self) -> String {
        let mut s = format!(
            "font-family=\"{}\" font-size=\"{:.2}\" {}",
            escapar(&self.familia), self.tamanho, preencher(self.cor)
        );
        if self.negrito {
            s.push_str(" font-weight=\"bold\"");
        }
        if self.italico {
            s.push_str(" font-style=\"italic\"");
        }
        s
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Ancora {
    Inicio,
    Meio,
    Fim,
}

struct Desenho {
    defs: String,
    corpo: String,
    mapa: MapaGrafico,
    /// Contorno da cor do fundo em volta do texto (rótulos que caem por
    /// cima de linhas e barras continuam legíveis).
    halo: Option<Cor>,
}

/// Uma linha da dica do mouse: o nome, o valor e se o valor é data.
#[derive(Debug, Clone, PartialEq)]
pub struct LinhaDica {
    pub rotulo: String,
    pub valor: f64,
    pub data: bool,
}

/// O que a dica mostra no ponto: título (a categoria) e as linhas.
#[derive(Debug, Clone, PartialEq)]
pub struct ConteudoDica {
    pub titulo: String,
    pub linhas: Vec<LinhaDica>,
}

/// Número de série do Excel (dias desde 30/12/1899) como dd/mm/aaaa.
pub fn data_de_serie(serie: f64) -> String {
    let dias = serie.floor() as i64 - 25_569 + 719_468;
    let era = dias.div_euclid(146_097);
    let doe = dias.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let dia = doy - (153 * mp + 2) / 5 + 1;
    let mes = if mp < 10 { mp + 3 } else { mp - 9 };
    let ano = yoe + era * 400 + i64::from(mes <= 2);
    format!("{dia:02}/{mes:02}/{ano}")
}

/// Onde ficou cada parte clicável do gráfico desenhado, em unidades do SVG
/// (as mesmas de `largura` × `altura`). A interface converte o clique/mouse
/// para essas unidades e pergunta aqui; nada precisa sair do SVG.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct MapaGrafico {
    /// Itens da legenda: índice (série; na pizza, categoria) e retângulo.
    pub legenda: Vec<AlvoLegenda>,
    /// Pontos de dados, na ordem do desenho (o último fica por cima).
    pub pontos: Vec<AlvoPonto>,
    /// O que a dica mostra quando o ponto não é um valor das séries de
    /// entrada (faixas do histograma, duração no Gantt, velas…).
    pub dica: Option<DadosDica>,
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AlvoLegenda {
    pub indice: usize,
    pub x: f32,
    pub y: f32,
    pub largura: f32,
    pub altura: f32,
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AlvoPonto {
    pub serie: usize,
    pub ponto: usize,
    pub forma: FormaAlvo,
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FormaAlvo {
    Retangulo { x: f32, y: f32, largura: f32, altura: f32 },
    /// Marcador de linha/dispersão: centro e raio de toque.
    Circulo { x: f32, y: f32, raio: f32 },
    /// Fatia de pizza: centro, raio e ângulos (radianos, horário a partir do eixo X).
    /// Na rosca, `raio_interno` é a borda de dentro do anel (0 na pizza).
    Fatia { x: f32, y: f32, raio_interno: f32, raio: f32, inicio: f32, fim: f32 },
}
impl FormaAlvo {
    fn contem(&self, px: f32, py: f32) -> bool {
        match *self {
            Self::Retangulo { x, y, largura, altura } => {
                // Barra fina demais: toque mínimo de 4 unidades.
                let (mx, my) = ((4.0 - largura).max(0.0) / 2.0, (4.0 - altura).max(0.0) / 2.0);
                px >= x - mx && px <= x + largura + mx && py >= y - my && py <= y + altura + my
            }
            Self::Circulo { x, y, raio } => (px - x).powi(2) + (py - y).powi(2) <= raio * raio,
            Self::Fatia { x, y, raio_interno, raio, inicio, fim } => {
                let (dx, dy) = (px - x, py - y);
                let d2 = dx * dx + dy * dy;
                if d2 > raio * raio || d2 < raio_interno * raio_interno {
                    return false;
                }
                let tau = std::f32::consts::TAU;
                let a = (dy.atan2(dx) - inicio).rem_euclid(tau);
                a <= (fim - inicio).max(0.0)
            }
        }
    }
}
impl MapaGrafico {
    /// O item da legenda sob o ponto, se houver.
    pub fn item_legenda_em(&self, x: f32, y: f32) -> Option<usize> {
        self.legenda.iter().find(|a| x >= a.x && x <= a.x + a.largura && y >= a.y && y <= a.y + a.altura).map(|a| a.indice)
    }
    /// O ponto de dados sob o mouse (o de cima, quando se sobrepõem).
    /// O conteúdo da dica no ponto: cada tipo diz o que lhe cabe (Gantt:
    /// início, término e duração; velas: abertura, máxima, mínima e
    /// fechamento; cascata: variação e acumulado; bolhas: Y e tamanho;
    /// histograma: a frequência da faixa). Os demais: a série e o valor.
    pub fn conteudo_dica(&self, g: &crate::Grafico, alvo: &AlvoPonto) -> Option<ConteudoDica> {
        let p = alvo.ponto;
        if let Some(d) = &self.dica {
            let linhas: Vec<LinhaDica> = d.series.iter().enumerate()
                .filter_map(|(i, (nome, valores))| Some(LinhaDica {
                    rotulo: nome.clone(),
                    valor: *valores.get(p)?,
                    data: d.datas.get(i).copied().unwrap_or(false),
                }))
                .collect();
            if linhas.is_empty() {
                return None;
            }
            return Some(ConteudoDica { titulo: d.categorias.get(p).cloned().unwrap_or_default(), linhas });
        }
        let serie = g.series.get(alvo.serie)?;
        Some(ConteudoDica {
            titulo: g.categorias.get(p).cloned().unwrap_or_default(),
            linhas: vec![LinhaDica { rotulo: serie.nome.clone(), valor: *serie.valores.get(p)?, data: false }],
        })
    }

    pub fn ponto_em(&self, x: f32, y: f32) -> Option<AlvoPonto> {
        self.pontos.iter().rev().find(|a| a.forma.contem(x, y)).copied()
    }
}
impl Desenho {
    #[allow(clippy::too_many_arguments)]
    fn texto(&mut self, x: f32, y: f32, t: &str, f: &Fonte, ancora: Ancora, giro: f32, classe: &str) {
        if t.is_empty() {
            return;
        }
        let a = match ancora {
            Ancora::Inicio => "start",
            Ancora::Meio => "middle",
            Ancora::Fim => "end",
        };
        let giro = if giro.abs() > 0.01 { format!(" transform=\"rotate({giro:.1} {x:.2} {y:.2})\"") } else { String::new() };
        let _ = writeln!(
            self.corpo,
            "<text class=\"{classe}\" x=\"{x:.2}\" y=\"{y:.2}\" text-anchor=\"{a}\" {}{giro}{}>{}</text>",
            f.atributos(),
            self.halo.map_or_else(String::new, |c| format!(" paint-order=\"stroke\" stroke=\"{}\" stroke-width=\"3\" stroke-linejoin=\"round\"", hex6(c))),
            escapar(t)
        );
    }
    #[allow(clippy::too_many_arguments)]
    fn linha(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, cor: Cor, esp: f32, traco: Traco, classe: &str) {
        let _ = writeln!(
            self.corpo,
            "<line class=\"{classe}\" x1=\"{x1:.2}\" y1=\"{y1:.2}\" x2=\"{x2:.2}\" y2=\"{y2:.2}\" {}/>",
            tracar(cor, esp, traco)
        );
    }
    #[allow(clippy::too_many_arguments)]
    /// Retângulo de área: preenchimento (sólido/degradê), borda, sombra, raio.
    fn area(&mut self, r: Ret, id: &str, preench: Option<&Preenchimento>, padrao: Option<Cor>, borda: Option<(Cor, f32, Traco)>, sombra: Option<&Sombra>, raio: f32) {
        let fill = match preench {
            Some(Preenchimento::Nenhum) => "fill=\"none\"".to_owned(),
            Some(Preenchimento::Solido { cor }) => preencher(cor.0),
            Some(Preenchimento::Degrade { inicio, fim, angulo }) => {
                let ang = angulo.to_radians();
                let (dx, dy) = (ang.cos() / 2.0, ang.sin() / 2.0);
                let _ = write!(
                    self.defs,
                    "<linearGradient id=\"grad-{id}\" x1=\"{:.3}\" y1=\"{:.3}\" x2=\"{:.3}\" y2=\"{:.3}\">\
                     <stop offset=\"0\" stop-color=\"{}\" stop-opacity=\"{:.3}\"/>\
                     <stop offset=\"1\" stop-color=\"{}\" stop-opacity=\"{:.3}\"/></linearGradient>",
                    0.5 - dx, 0.5 - dy, 0.5 + dx, 0.5 + dy,
                    hex6(inicio.0), alfa(inicio.0), hex6(fim.0), alfa(fim.0)
                );
                format!("fill=\"url(#grad-{id})\"")
            }
            None => padrao.map_or_else(|| "fill=\"none\"".to_owned(), preencher),
        };
        let filtro = match sombra {
            Some(s) => {
                let _ = write!(
                    self.defs,
                    "<filter id=\"sombra-{id}\" x=\"-20%\" y=\"-20%\" width=\"140%\" height=\"140%\">\
                     <feDropShadow dx=\"{:.2}\" dy=\"{:.2}\" stdDeviation=\"{:.2}\" flood-color=\"{}\" flood-opacity=\"{:.3}\"/></filter>",
                    s.dx, s.dy, (s.desfoque / 2.0).max(0.0), hex6(s.cor.0), alfa(s.cor.0)
                );
                format!(" filter=\"url(#sombra-{id})\"")
            }
            None => String::new(),
        };
        let traco = borda.map_or_else(String::new, |(c, e, t)| format!(" {}", tracar(c, e, t)));
        let _ = writeln!(
            self.corpo,
            "<rect class=\"area-{id}\" x=\"{:.2}\" y=\"{:.2}\" width=\"{:.2}\" height=\"{:.2}\" rx=\"{raio:.2}\" {fill}{traco}{filtro}/>",
            r.x, r.y, r.w, r.h
        );
    }
}

fn formatar(v: f64, f: FormatoNum, casas_auto: u8) -> String {
    let fmt = match f {
        FormatoNum::Automatico => FormatoNumero::Decimal(casas_auto.min(6)),
        FormatoNum::Decimal { casas } => FormatoNumero::Decimal(casas.min(6)),
        FormatoNum::Moeda { casas } => FormatoNumero::Moeda(casas.min(6)),
        FormatoNum::Percentual { casas } => FormatoNumero::Percentual(casas.min(6)),
    };
    fmt.formatar(v, LocalidadeGrafico::PtBr).unwrap_or_else(|_| format!("{v}"))
}
impl FormatoNum {
    /// O número como o gráfico mostra (pt-BR), para a dica do mouse e afins.
    pub fn formatar(self, v: f64) -> String {
        formatar(v, self, casas_de(v))
    }
}
/// Casas para que `v` apareça sem arredondar demais (até 2).
fn casas_de(v: f64) -> u8 {
    if (v - v.round()).abs() < 1e-9 {
        0
    } else if (v * 10.0 - (v * 10.0).round()).abs() < 1e-9 {
        1
    } else {
        2
    }
}
fn casas_do_passo(passo: f64) -> u8 {
    if passo >= 1.0 && (passo - passo.round()).abs() < 1e-9 {
        0
    } else {
        ((-passo.log10()).ceil().max(0.0) as u8 + u8::from(casas_de(passo / 10f64.powf(passo.log10().floor())) > 0)).min(6)
    }
}

fn passo_bonito(intervalo: f64, alvo: f64) -> f64 {
    let bruto = (intervalo / alvo).abs().max(f64::MIN_POSITIVE);
    let mag = 10f64.powf(bruto.log10().floor());
    let n = bruto / mag;
    let f = if n <= 1.0 { 1.0 } else if n <= 2.0 { 2.0 } else if n <= 2.5 { 2.5 } else if n <= 5.0 { 5.0 } else { 10.0 };
    f * mag
}

// ------------------------------------------------------------------ escala

#[derive(Debug, Clone)]
struct Escala {
    min: f64,
    max: f64,
    log: bool,
    inv: bool,
    ticks: Vec<f64>,
    menores: Vec<f64>,
    casas: u8,
}
impl Escala {
    fn nova(dmin: f64, dmax: f64, e: &Eixo, incluir_zero: bool) -> Escala {
        let (dmin, dmax) = if dmin.is_finite() && dmax.is_finite() { (dmin.min(dmax), dmax.max(dmin)) } else { (0.0, 1.0) };
        if e.logaritmica {
            let pmin = if dmin > 0.0 { dmin } else if dmax > 0.0 { dmax / 100.0 } else { 1.0 };
            let pmax = dmax.max(pmin * 10.0);
            let min = e.minimo.filter(|v| *v > 0.0).unwrap_or_else(|| 10f64.powf(pmin.log10().floor()));
            let mut max = e.maximo.filter(|v| *v > min).unwrap_or_else(|| 10f64.powf(pmax.log10().ceil()));
            if max <= min {
                max = min * 10.0;
            }
            let base = e.unidade_principal.filter(|u| *u > 1.0).unwrap_or(10.0);
            let mut ticks = Vec::new();
            let mut t = base.powf((min.ln() / base.ln()).ceil());
            while t <= max * (1.0 + 1e-9) && ticks.len() < 60 {
                ticks.push(t);
                t *= base;
            }
            let mut menores = Vec::new();
            let mut d = 10f64.powf(min.log10().floor());
            while d < max && menores.len() < 600 {
                for k in 2..10 {
                    let v = d * f64::from(k);
                    if v > min && v < max {
                        menores.push(v);
                    }
                }
                d *= 10.0;
            }
            let casas = ticks.iter().map(|v| casas_de(*v)).max().unwrap_or(0);
            return Escala { min, max, log: true, inv: e.invertido, ticks, menores, casas };
        }
        let (mut lo, mut hi) = if incluir_zero { (dmin.min(0.0), dmax.max(0.0)) } else { (dmin, dmax) };
        if let Some(m) = e.minimo {
            lo = m;
        }
        if let Some(m) = e.maximo {
            hi = m;
        }
        if hi <= lo {
            hi = lo + if lo == 0.0 { 1.0 } else { lo.abs() };
        }
        let mut passo = e.unidade_principal.filter(|u| *u > 0.0).unwrap_or_else(|| passo_bonito(hi - lo, 5.0));
        if (hi - lo) / passo > 200.0 {
            passo = passo_bonito(hi - lo, 200.0);
        }
        let min = e.minimo.unwrap_or_else(|| (lo / passo).floor() * passo);
        let mut max = e.maximo.unwrap_or_else(|| (hi / passo).ceil() * passo);
        if max <= min {
            max = min + passo;
        }
        let mut ticks = Vec::new();
        let mut k = (min / passo - 1e-9).ceil();
        while k * passo <= max + passo * 1e-9 && ticks.len() < 201 {
            let v = k * passo;
            ticks.push(if v.abs() < passo * 1e-9 { 0.0 } else { v });
            k += 1.0;
        }
        let menor = e.unidade_secundaria.filter(|u| *u > 0.0).unwrap_or(passo / 5.0);
        let mut menores = Vec::new();
        if (max - min) / menor <= 1000.0 {
            let mut k = (min / menor).ceil();
            while k * menor < max && menores.len() < 1000 {
                menores.push(k * menor);
                k += 1.0;
            }
        }
        Escala { min, max, log: false, inv: e.invertido, ticks, menores, casas: casas_do_passo(passo) }
    }
    fn fracao(&self, v: f64) -> f32 {
        let t = if self.log {
            let v = v.max(self.min * 1e-6).max(f64::MIN_POSITIVE);
            (v.log10() - self.min.log10()) / (self.max.log10() - self.min.log10())
        } else {
            (v - self.min) / (self.max - self.min)
        };
        let t = if t.is_finite() { t.clamp(0.0, 1.0) as f32 } else { 0.0 };
        if self.inv { 1.0 - t } else { t }
    }
    /// Onde a outra linha (categorias ou barras) cruza este eixo.
    fn base(&self, cruzamento: Option<f64>) -> f64 {
        match cruzamento {
            Some(v) => v.clamp(self.min, self.max),
            None if self.log => self.min,
            None => 0.0f64.clamp(self.min, self.max),
        }
    }
}

// --------------------------------------------------------------- resolução

struct Resolvido<'a> {
    e: &'a EstiloGrafico,
    tema: Option<&'static TemaGrafico>,
    base: Fonte,
}
impl Resolvido<'_> {
    fn ativo(&self, i: usize) -> bool {
        !self.e.legenda.desligado(i)
    }
    fn eixo_cor(&self) -> Cor {
        self.tema.map_or(EIXO_PADRAO, |t| t.eixo.0)
    }
    fn cor_serie(&self, g: &Grafico, i: usize) -> Cor {
        let s = self.e.serie(i);
        let c = s.and_then(|s| s.cor).map(|c| c.0)
            .or_else(|| self.tema.map(|t| t.paleta[i % t.paleta.len()].0))
            .unwrap_or_else(|| g.series.get(i).map_or(TEXTO_PADRAO, |s| s.cor));
        com_alfa(c, s.and_then(|s| s.opacidade).unwrap_or(1.0))
    }
    fn cor_ponto(&self, g: &Grafico, serie: usize, ponto: usize) -> Cor {
        let s = self.e.serie(serie);
        let op = s.and_then(|s| s.opacidade).unwrap_or(1.0);
        // Na rosca a cor da fatia vale para todos os anéis: a da 1ª série, se o anel não tiver a sua.
        let pintado = |i: usize| self.e.serie(i).and_then(|s| s.pontos.get(&ponto)).and_then(|p| p.cor);
        let cor = pintado(serie).or_else(|| if g.tipo == Tipo::Pizza { pintado(0) } else { None });
        match cor {
            Some(c) => com_alfa(c.0, op),
            None if g.tipo == Tipo::Pizza => {
                let c = self.tema.map_or(PIZZA_PADRAO[ponto % PIZZA_PADRAO.len()], |t| t.paleta[ponto % t.paleta.len()].0);
                com_alfa(c, op)
            }
            None => self.cor_serie(g, serie),
        }
    }
    fn linha(&self, l: &Linha, visivel: bool, cor: Cor, esp: f32) -> Option<(Cor, f32, Traco)> {
        if !l.visivel.unwrap_or(visivel) {
            return None;
        }
        Some((l.cor.map_or(cor, |c| c.0), l.espessura.filter(|v| v.is_finite() && *v >= 0.0).unwrap_or(esp), l.traco.unwrap_or_default()))
    }
    fn rotulos_da_serie(&self, i: usize) -> Rotulos {
        let g = &self.e.rotulos;
        let s = self.e.serie(i).map(|s| &s.rotulos);
        let pega = |f: fn(&Rotulos) -> Option<bool>| s.and_then(f).or_else(|| f(g));
        Rotulos {
            visivel: pega(|r| r.visivel),
            posicao: s.and_then(|r| r.posicao).or(g.posicao),
            formato: s.and_then(|r| r.formato).or(g.formato),
            fonte: s.map_or_else(|| g.fonte.clone(), |r| if r.fonte == Texto::default() { g.fonte.clone() } else { r.fonte.clone() }),
            valor: pega(|r| r.valor),
            nome_serie: pega(|r| r.nome_serie),
            categoria: pega(|r| r.categoria),
            percentual: pega(|r| r.percentual),
        }
    }
}

fn caminho(pontos: &[(f32, f32)], suavizar: bool) -> String {
    let mut d = String::new();
    let Some(p0) = pontos.first() else { return d };
    let _ = write!(d, "M{:.2} {:.2}", p0.0, p0.1);
    if !suavizar || pontos.len() < 3 {
        for p in &pontos[1..] {
            let _ = write!(d, " L{:.2} {:.2}", p.0, p.1);
        }
        return d;
    }
    for i in 0..pontos.len() - 1 {
        let _ = write!(d, " {}", trecho_suave(pontos, i));
    }
    d
}
/// Bézier cúbica de Catmull-Rom do ponto `i` ao `i + 1`.
fn trecho_suave(p: &[(f32, f32)], i: usize) -> String {
    let a = p[i.saturating_sub(1)];
    let b = p[i];
    let c = p[i + 1];
    let d = p[(i + 2).min(p.len() - 1)];
    let c1 = (b.0 + (c.0 - a.0) / 6.0, b.1 + (c.1 - a.1) / 6.0);
    let c2 = (c.0 - (d.0 - b.0) / 6.0, c.1 - (d.1 - b.1) / 6.0);
    format!("C{:.2} {:.2} {:.2} {:.2} {:.2} {:.2}", c1.0, c1.1, c2.0, c2.1, c.0, c.1)
}

fn marcador(d: &mut Desenho, forma: FormaMarcador, x: f32, y: f32, tam: f32, cor: Cor, contorno: Option<(Cor, f32)>) {
    let r = tam / 2.0;
    let traco = contorno.map_or_else(String::new, |(c, e)| format!(" {}", tracar(c, e, Traco::Continuo)));
    let fill = preencher(cor);
    let _ = match forma {
        FormaMarcador::Nenhum => Ok(()),
        FormaMarcador::Circulo => writeln!(d.corpo, "<circle class=\"marcador\" cx=\"{x:.2}\" cy=\"{y:.2}\" r=\"{r:.2}\" {fill}{traco}/>"),
        FormaMarcador::Quadrado => writeln!(d.corpo, "<rect class=\"marcador\" x=\"{:.2}\" y=\"{:.2}\" width=\"{tam:.2}\" height=\"{tam:.2}\" {fill}{traco}/>", x - r, y - r),
        FormaMarcador::Losango => writeln!(d.corpo, "<path class=\"marcador\" d=\"M{x:.2} {:.2} L{:.2} {y:.2} L{x:.2} {:.2} L{:.2} {y:.2} Z\" {fill}{traco}/>", y - r, x + r, y + r, x - r),
        FormaMarcador::Triangulo => writeln!(d.corpo, "<path class=\"marcador\" d=\"M{x:.2} {:.2} L{:.2} {:.2} L{:.2} {:.2} Z\" {fill}{traco}/>", y - r, x + r, y + r * 0.8, x - r, y + r * 0.8),
        FormaMarcador::Cruz => writeln!(d.corpo, "<path class=\"marcador\" d=\"M{:.2} {:.2} L{:.2} {:.2} M{:.2} {:.2} L{:.2} {:.2}\" {}/>", x - r, y - r, x + r, y + r, x - r, y + r, x + r, y - r, tracar(cor, (tam / 4.0).max(1.0), Traco::Continuo)),
    };
}

// ------------------------------------------------------------------ desenho

impl Grafico {
    /// SVG do gráfico com o estilo completo. Os dados não são alterados.
    pub fn exportar_svg_formatado(&self, largura: f32, altura: f32, e: &EstiloGrafico) -> Result<String, ErroGrafico> {
        self.desenhar_formatado(largura, altura, e).map(|(svg, _)| svg)
    }

    /// O SVG e o mapa das partes clicáveis (legenda, pontos), do mesmo desenho.
    pub fn exportar_svg_formatado_com_mapa(&self, largura: f32, altura: f32, e: &EstiloGrafico) -> Result<(String, MapaGrafico), ErroGrafico> {
        self.desenhar_formatado(largura, altura, e)
    }

    fn desenhar_formatado(&self, largura: f32, altura: f32, e: &EstiloGrafico) -> Result<(String, MapaGrafico), ErroGrafico> {
        self.desenhar_com(largura, altura, e, None)
    }

    /// Os tipos que não cabem no desenho cartesiano comum (radar, cascata,
    /// histograma, combinado, bolhas, velas, Gantt, mapa de calor, linha 3D),
    /// com a mesma moldura: fundo, título, legenda, tema e fonte do estilo.
    pub fn exportar_svg_avancado_com_mapa(&self, largura: f32, altura: f32, e: &EstiloGrafico, pedido: &PedidoAvancado) -> Result<(String, MapaGrafico), ErroGrafico> {
        pedido.validar(self)?;
        let mut g = self.clone();
        g.tipo = Tipo::Colunas;
        g.desenhar_com(largura, altura, e, Some(pedido))
    }

    fn desenhar_com(&self, largura: f32, altura: f32, e: &EstiloGrafico, avancado: Option<&PedidoAvancado>) -> Result<(String, MapaGrafico), ErroGrafico> {
        if self.tipo == Tipo::Pizza && e.rosca.is_some() {
            self.validar_rosca()?;
        } else {
            self.validar()?;
        }
        if !largura.is_finite() || !altura.is_finite() || largura < 20.0 || altura < 20.0 {
            return Err(ErroGrafico::EixoInvalido);
        }
        if !e.valido() {
            return Err(ErroGrafico::ValorInvalido);
        }
        let tema = e.tema();
        let base = Fonte {
            familia: e.fonte.familia.clone().filter(|f| !f.trim().is_empty())
                .unwrap_or_else(|| tema.map_or(FAMILIA_PADRAO, |t| t.familia).to_owned()),
            tamanho: e.fonte.tamanho.filter(|v| v.is_finite() && *v > 0.0).map_or(12.0, |v| v.clamp(4.0, 72.0)),
            cor: e.fonte.cor.map(|c| c.0).or(tema.map(|t| t.texto.0)).unwrap_or(TEXTO_PADRAO),
            negrito: e.fonte.negrito.unwrap_or(false),
            italico: e.fonte.italico.unwrap_or(false),
        };
        let r = Resolvido { e, tema, base };
        let mut d = Desenho { defs: String::new(), corpo: String::new(), mapa: MapaGrafico::default(), halo: None };
        let (w, h) = (largura, altura);

        // Área do gráfico.
        let borda_padrao = tema.and_then(|t| t.borda).map(|c| c.0);
        let borda = r.linha(&e.area_grafico.borda, borda_padrao.is_some(), borda_padrao.unwrap_or(EIXO_PADRAO), 1.0);
        let sombra_tema = tema.filter(|t| t.sombra).map(|_| Sombra::default());
        let raio = e.area_grafico.raio.unwrap_or(tema.map_or(0.0, |t| t.raio));
        let meia = borda.map_or(0.0, |b| b.1 / 2.0);
        d.area(
            Ret { x: meia, y: meia, w: w - 2.0 * meia, h: h - 2.0 * meia }, "grafico",
            e.area_grafico.preenchimento.as_ref(), Some(tema.map_or(Cor::rgba(255, 255, 255, 255), |t| t.fundo.0)),
            borda, e.area_grafico.sombra.as_ref().or(sombra_tema.as_ref()), raio,
        );

        // Caixa de conteúdo (margens do gráfico).
        let mg = &e.margens_grafico;
        let padrao_mg = 10.0;
        let mut caixa = Ret { x: 0.0, y: 0.0, w, h }.encolher(
            mg.superior.unwrap_or(padrao_mg), mg.inferior.unwrap_or(padrao_mg),
            mg.esquerda.unwrap_or(padrao_mg), mg.direita.unwrap_or(padrao_mg),
        );

        // Título e subtítulo.
        let f_titulo = Fonte::de(&r.base, &e.titulo.fonte, 1.4);
        let f_sub = Fonte::de(&r.base, &e.subtitulo.fonte, 1.0);
        let texto_titulo = e.titulo.texto.clone().unwrap_or_else(|| self.titulo.clone());
        let mostra_titulo = e.titulo.visivel.unwrap_or(true) && !texto_titulo.trim().is_empty();
        let texto_sub = e.subtitulo.texto.clone().unwrap_or_default();
        let (tx, ancora_t) = match e.titulo.alinhamento.unwrap_or_default() {
            Alinhamento::Esquerda => (caixa.x, Ancora::Inicio),
            Alinhamento::Centro => (caixa.x + caixa.w / 2.0, Ancora::Meio),
            Alinhamento::Direita => (caixa.x + caixa.w, Ancora::Fim),
        };
        let mut topo = caixa.y;
        if mostra_titulo {
            topo += f_titulo.tamanho * 0.95;
            let t = cortar(&texto_titulo, caixa.w, f_titulo.tamanho);
            d.texto(tx, topo, &t, &f_titulo, ancora_t, 0.0, "titulo");
            topo += f_titulo.tamanho * 0.35;
        }
        if !texto_sub.trim().is_empty() {
            topo += f_sub.tamanho * 1.1;
            let t = cortar(&texto_sub, caixa.w, f_sub.tamanho);
            d.texto(tx, topo, &t, &f_sub, ancora_t, 0.0, "subtitulo");
            topo += f_sub.tamanho * 0.3;
        }
        if topo > caixa.y && !e.titulo.sobreposto {
            caixa = caixa.encolher(topo - caixa.y + 6.0, 0.0, 0.0, 0.0);
        }

        // Legenda (itens = séries; na pizza, categorias).
        let pizza = self.tipo == Tipo::Pizza;
        let f_leg = Fonte::de(&r.base, &e.legenda.fonte, 1.0);
        let total_itens = if pizza { self.categorias.len() } else { self.series.len() };
        let posicao_leg = e.legenda.posicao.unwrap_or_default();
        let com_legenda = avancado.map_or(true, |p| p.tipo.tem_legenda());
        let so_estas = avancado.and_then(|p| p.tipo.itens_legenda(self.series.len()));
        let itens: Vec<(usize, String, Cor)> = if posicao_leg == PosicaoLegenda::Nenhuma || !com_legenda {
            Vec::new()
        } else {
            e.legenda.ordem_visivel(total_itens).into_iter()
                .filter(|i| so_estas.as_ref().map_or(true, |l| l.contains(i))).map(|i| {
                let item = e.legenda.itens.get(&i);
                let nome = item.and_then(|it| it.texto.clone()).unwrap_or_else(|| {
                    if pizza { self.categorias[i].clone() } else { self.series[i].nome.clone() }
                });
                let cor = item.and_then(|it| it.cor).map(|c| c.0)
                    .unwrap_or_else(|| if pizza { r.cor_ponto(self, 0, i) } else { r.cor_serie(self, i) });
                (i, nome, cor)
            }).filter(|(_, n, _)| !n.trim().is_empty()).collect()
        };
        let fs = f_leg.tamanho;
        let linha_serie = matches!(self.tipo, Tipo::Linhas | Tipo::Degraus) || avancado.is_some_and(|p| p.tipo == TipoAvancado::Linha3d);
        let amostra_w = if linha_serie { fs * 1.6 } else { fs * 0.8 };
        let item_w = |n: &str| amostra_w + fs * 0.4 + largura_texto(n, fs) + fs * 1.0;
        let mut desenhar_legenda: Vec<(f32, f32, usize, String, Cor)> = Vec::new();
        if !itens.is_empty() {
            match posicao_leg {
                PosicaoLegenda::Inferior | PosicaoLegenda::Superior => {
                    let mut linhas: Vec<Vec<(usize, String, Cor, f32)>> = vec![Vec::new()];
                    let mut usado = 0.0;
                    for (i, n, c) in &itens {
                        let nome = cortar(n, caixa.w * 0.6, fs);
                        let lw = item_w(&nome);
                        if usado + lw > caixa.w && !linhas.last().unwrap().is_empty() {
                            linhas.push(Vec::new());
                            usado = 0.0;
                        }
                        usado += lw;
                        linhas.last_mut().unwrap().push((*i, nome, *c, lw));
                    }
                    let alt = linhas.len() as f32 * fs * 1.45;
                    let y0 = if posicao_leg == PosicaoLegenda::Inferior { caixa.y + caixa.h - alt } else { caixa.y };
                    for (k, l) in linhas.iter().enumerate() {
                        let total: f32 = l.iter().map(|x| x.3).sum::<f32>() - fs;
                        let mut x = caixa.x + (caixa.w - total).max(0.0) / 2.0;
                        let y = y0 + fs * 1.05 + k as f32 * fs * 1.45;
                        for (i, n, c, lw) in l {
                            desenhar_legenda.push((x, y, *i, n.clone(), *c));
                            x += lw;
                        }
                    }
                    caixa = if posicao_leg == PosicaoLegenda::Inferior {
                        caixa.encolher(0.0, alt + 4.0, 0.0, 0.0)
                    } else {
                        caixa.encolher(alt + 4.0, 0.0, 0.0, 0.0)
                    };
                }
                PosicaoLegenda::Esquerda | PosicaoLegenda::Direita => {
                    let maximo = caixa.w * 0.35;
                    let nomes: Vec<String> = itens.iter().map(|(_, n, _)| cortar(n, maximo - amostra_w - fs, fs)).collect();
                    let lw = nomes.iter().map(|n| item_w(n)).fold(0.0, f32::max).min(maximo);
                    let alt = itens.len() as f32 * fs * 1.45;
                    let y0 = caixa.y + ((caixa.h - alt) / 2.0).max(0.0);
                    let x = if posicao_leg == PosicaoLegenda::Esquerda { caixa.x } else { caixa.x + caixa.w - lw + fs };
                    for (k, ((i, _, c), n)) in itens.iter().zip(nomes).enumerate() {
                        desenhar_legenda.push((x, y0 + fs * 1.05 + k as f32 * fs * 1.45, *i, n, *c));
                    }
                    caixa = if posicao_leg == PosicaoLegenda::Esquerda {
                        caixa.encolher(0.0, 0.0, lw + 4.0, 0.0)
                    } else {
                        caixa.encolher(0.0, 0.0, 0.0, lw + 4.0)
                    };
                }
                PosicaoLegenda::Nenhuma => {}
            }
        }
        if desenhar_legenda.iter().any(|(_, _, i, _, _)| e.legenda.desligado(*i)) {
            // Amostra do item desligado: hachura cinza (como no Excel/Sheets).
            let _ = write!(d.defs, "<pattern id=\"hachura-legenda\" width=\"4\" height=\"4\" patternUnits=\"userSpaceOnUse\" patternTransform=\"rotate(45)\">\
                <rect width=\"4\" height=\"4\" fill=\"#ffffff\" fill-opacity=\"0\"/><line x1=\"0\" y1=\"0\" x2=\"0\" y2=\"4\" stroke=\"#9aa0a6\" stroke-width=\"1.6\"/></pattern>");
        }
        for (x, y, i, nome, cor) in &desenhar_legenda {
            let desligado = e.legenda.desligado(*i);
            if linha_serie {
                let (c, t) = if desligado { (Cor::rgba(154, 160, 166, 255), Traco::Tracejado) } else { (*cor, Traco::Continuo) };
                d.linha(*x, y - fs * 0.35, x + amostra_w, y - fs * 0.35, c, 3.0, t, "legenda-amostra");
            } else if desligado {
                let _ = writeln!(d.corpo, "<rect class=\"legenda-amostra desligado\" x=\"{x:.2}\" y=\"{:.2}\" width=\"{:.2}\" height=\"{:.2}\" rx=\"1.5\" fill=\"url(#hachura-legenda)\" stroke=\"#9aa0a6\" stroke-width=\"0.8\"/>",
                    y - fs * 0.78, fs * 0.8, fs * 0.8);
            } else {
                let _ = writeln!(d.corpo, "<rect class=\"legenda-amostra\" x=\"{x:.2}\" y=\"{:.2}\" width=\"{:.2}\" height=\"{:.2}\" rx=\"1.5\" {}/>",
                    y - fs * 0.78, fs * 0.8, fs * 0.8, preencher(*cor));
            }
            let mut f = f_leg.clone();
            if desligado {
                f.cor = Cor::rgba(154, 160, 166, 255);
            }
            d.texto(x + amostra_w + fs * 0.4, *y, nome, &f, Ancora::Inicio, 0.0, "legenda");
            d.mapa.legenda.push(AlvoLegenda {
                indice: *i,
                x: x - fs * 0.25,
                y: y - fs * 1.05,
                largura: amostra_w + fs * 0.65 + largura_texto(nome, fs),
                altura: fs * 1.4,
            });
        }

        if let Some(p) = avancado {
            avancado::desenhar(self, &mut d, &r, caixa, p);
        } else if pizza {
            self.desenhar_pizza(&mut d, &r, caixa);
        } else {
            self.desenhar_cartesiano(&mut d, &r, caixa);
        }

        let mut svg = format!(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" version=\"1.1\" width=\"{w}\" height=\"{h}\" viewBox=\"0 0 {w} {h}\" role=\"img\"{}>\n",
            if e.texto_alternativo.trim().is_empty() { String::new() } else { format!(" aria-label=\"{}\"", escapar(&e.texto_alternativo)) }
        );
        if !e.texto_alternativo.trim().is_empty() {
            let _ = writeln!(svg, "<title>{}</title><desc>{}</desc>", escapar(&texto_titulo), escapar(&e.texto_alternativo));
        }
        if !d.defs.is_empty() {
            let _ = writeln!(svg, "<defs>{}</defs>", d.defs);
        }
        svg.push_str(&d.corpo);
        svg.push_str("</svg>\n");
        Ok((svg, d.mapa))
    }

    fn desenhar_pizza(&self, d: &mut Desenho, r: &Resolvido<'_>, caixa: Ret) {
        let e = r.e;
        let mp = &e.margens_plotagem;
        let area = caixa.encolher(mp.superior.unwrap_or(0.0), mp.inferior.unwrap_or(0.0), mp.esquerda.unwrap_or(0.0), mp.direita.unwrap_or(0.0));
        if e.area_plotagem != Area::default() || r.tema.and_then(|t| t.plotagem).is_some() {
            let b = r.linha(&e.area_plotagem.borda, false, r.eixo_cor(), 1.0);
            d.area(caixa, "plotagem", e.area_plotagem.preenchimento.as_ref(), r.tema.and_then(|t| t.plotagem).map(|c| c.0),
                b, e.area_plotagem.sombra.as_ref(), e.area_plotagem.raio.unwrap_or(0.0));
        }
        let (cx, cy) = (area.x + area.w / 2.0, area.y + area.h / 2.0);
        let raio = (area.w.min(area.h) / 2.0 * 0.9).max(2.0);
        // Rosca: um anel por série, a primeira por dentro (como no Excel).
        let furo = e.rosca.filter(|f| f.is_finite()).map_or(0.0, |f| f.clamp(0.0, 0.9)) * raio;
        let aneis = if furo > 0.0 { self.series.len() } else { 1 };
        let espessura = (raio - furo) / aneis as f32;
        for s in 0..aneis {
            let r_in = furo + espessura * s as f32;
            self.desenhar_anel(d, r, s, (cx, cy), (r_in, r_in + espessura), s + 1 == aneis);
        }
    }

    /// Um anel da série `s` (`r_in` = 0: a pizza inteira). `externo`: o anel
    /// de fora, o único que pode pôr rótulo do lado de fora.
    fn desenhar_anel(&self, d: &mut Desenho, r: &Resolvido<'_>, s: usize, (cx, cy): (f32, f32), (r_in, raio): (f32, f32), externo: bool) {
        let e = r.e;
        let valores = &self.series[s].valores;
        let soma: f64 = valores.iter().enumerate().filter(|(i, v)| **v > 0.0 && r.ativo(*i)).map(|(_, v)| *v).sum();
        let estilo = e.serie(s);
        let contorno = estilo.and_then(|s| s.contorno).map(|c| c.0);
        let esp_contorno = estilo.and_then(|s| s.espessura_contorno).unwrap_or(1.0);
        let rot = r.rotulos_da_serie(s);
        let f_rot = Fonte::de(&r.base, &rot.fonte, 1.0);
        let mostra = rot.visivel.unwrap_or(true);
        // Com vários anéis o nome da categoria repetiria em cada um: só se pedido.
        let categoria_padrao = r_in <= 0.0 || self.series.len() == 1;
        let mut ang = -std::f32::consts::FRAC_PI_2;
        for (i, &v) in valores.iter().enumerate() {
            if v <= 0.0 || soma <= 0.0 || !r.ativo(i) {
                continue;
            }
            let frac = (v / soma) as f32;
            let arco = frac * std::f32::consts::TAU;
            let (a0, a1) = (ang, ang + arco);
            ang = a1;
            let meio = (a0 + a1) / 2.0;
            let ponto = estilo.and_then(|s| s.pontos.get(&i));
            let des = ponto.and_then(|p| p.destacar).unwrap_or(0.0).clamp(0.0, raio);
            let (ox, oy) = (cx + des * meio.cos(), cy + des * meio.sin());
            let cor = r.cor_ponto(self, s, i);
            d.mapa.pontos.push(AlvoPonto { serie: s, ponto: i, forma: FormaAlvo::Fatia { x: ox, y: oy, raio_interno: r_in, raio, inicio: a0, fim: a1 } });
            let traco = ponto.and_then(|p| p.contorno).map(|c| c.0).or(contorno)
                .map_or_else(String::new, |c| format!(" {}", tracar(c, esp_contorno, Traco::Continuo)));
            let p = |rr: f32, a: f32| (ox + rr * a.cos(), oy + rr * a.sin());
            let grande = u8::from(arco > std::f32::consts::PI);
            let ((x0, y0), (x1, y1)) = (p(raio, a0), p(raio, a1));
            if frac >= 0.9999 && r_in > 0.0 {
                // Anel inteiro: dois círculos, e o de dentro vaza (evenodd).
                let _ = writeln!(
                    d.corpo,
                    "<path class=\"fatia\" fill-rule=\"evenodd\" d=\"M{:.2} {oy:.2} A{raio:.2} {raio:.2} 0 1 1 {:.2} {oy:.2} A{raio:.2} {raio:.2} 0 1 1 {:.2} {oy:.2} Z M{:.2} {oy:.2} A{r_in:.2} {r_in:.2} 0 1 0 {:.2} {oy:.2} A{r_in:.2} {r_in:.2} 0 1 0 {:.2} {oy:.2} Z\" {}{traco}/>",
                    ox + raio, ox - raio, ox + raio, ox + r_in, ox - r_in, ox + r_in, preencher(cor)
                );
            } else if frac >= 0.9999 {
                let _ = writeln!(d.corpo, "<circle class=\"fatia\" cx=\"{ox:.2}\" cy=\"{oy:.2}\" r=\"{raio:.2}\" {}{traco}/>", preencher(cor));
            } else if r_in > 0.0 {
                let ((x2, y2), (x3, y3)) = (p(r_in, a1), p(r_in, a0));
                let _ = writeln!(
                    d.corpo,
                    "<path class=\"fatia\" d=\"M{x0:.2} {y0:.2} A{raio:.2} {raio:.2} 0 {grande} 1 {x1:.2} {y1:.2} L{x2:.2} {y2:.2} A{r_in:.2} {r_in:.2} 0 {grande} 0 {x3:.2} {y3:.2} Z\" {}{traco}/>",
                    preencher(cor)
                );
            } else {
                let _ = writeln!(
                    d.corpo,
                    "<path class=\"fatia\" d=\"M{ox:.2} {oy:.2} L{x0:.2} {y0:.2} A{raio:.2} {raio:.2} 0 {grande} 1 {x1:.2} {y1:.2} Z\" {}{traco}/>",
                    preencher(cor)
                );
            }
            if mostra {
                let mut partes = Vec::new();
                if rot.categoria.unwrap_or(categoria_padrao) {
                    partes.push(self.categorias[i].clone());
                }
                if rot.valor.unwrap_or(false) {
                    partes.push(formatar(v, rot.formato.unwrap_or_default(), casas_de(v)));
                }
                if rot.percentual.unwrap_or(false) {
                    partes.push(formatar(f64::from(frac), FormatoNum::Percentual { casas: 1 }, 1));
                }
                let texto = partes.join("; ");
                // Na rosca, só o anel de fora põe rótulo fora; os outros ficam no meio do anel.
                let fora = match rot.posicao.unwrap_or_default() {
                    _ if r_in > 0.0 && !externo => false,
                    PosicaoRotulo::ForaFim => true,
                    PosicaoRotulo::Centro | PosicaoRotulo::DentroFim | PosicaoRotulo::DentroBase => false,
                    _ => frac < 0.07,
                };
                let dist = if fora {
                    raio * 1.12
                } else if r_in > 0.0 {
                    (r_in + raio) / 2.0
                } else {
                    raio * match rot.posicao.unwrap_or_default() {
                        PosicaoRotulo::Centro => 0.5,
                        PosicaoRotulo::DentroBase => 0.35,
                        _ => 0.65,
                    }
                };
                let (lx, ly) = p(dist, meio);
                let ancora = if !fora { Ancora::Meio } else if meio.cos() >= 0.0 { Ancora::Inicio } else { Ancora::Fim };
                d.texto(lx, ly + f_rot.tamanho * 0.35, &texto, &f_rot, ancora, 0.0, "rotulo");
            }
        }
    }

    #[allow(clippy::too_many_lines)]
    fn desenhar_cartesiano(&self, d: &mut Desenho, r: &Resolvido<'_>, caixa: Ret) {
        let e = r.e;
        let horizontal = self.tipo == Tipo::Barras;
        let dispersao = self.tipo == Tipo::Dispersao;
        let barras = matches!(self.tipo, Tipo::Colunas | Tipo::Barras);
        let n = self.categorias.len();
        // Séries desligadas pelo clique na legenda não entram no desenho nem na escala.
        let ligada = |s: usize| r.ativo(s);
        // Posição de cada série no grupo de barras, contando só as ligadas.
        let vagas: Vec<usize> = (0..self.series.len()).scan(0usize, |k, s| { let v = *k; if ligada(s) { *k += 1; } Some(v) }).collect();
        // Empilhadas: cada série começa onde a anterior terminou (positivos
        // para cima, negativos para baixo); em 100%, a parte do total.
        let empilhar = if dispersao { Empilhar::Nenhum } else { e.empilhar };
        let pilhas: Option<Vec<Vec<(f64, f64)>>> = (!empilhar.nenhum()).then(|| {
            let total: Vec<f64> = (0..n).map(|i| self.series.iter().enumerate().filter(|(k, _)| ligada(*k)).map(|(_, s)| s.valores.get(i).copied().unwrap_or(0.0).abs()).sum()).collect();
            let (mut pos, mut neg) = (vec![0.0f64; n], vec![0.0f64; n]);
            self.series.iter().enumerate().map(|(k, s)| (0..n).map(|i| {
                let mut v = if ligada(k) { s.valores.get(i).copied().unwrap_or(0.0) } else { 0.0 };
                if empilhar == Empilhar::Cem {
                    v = if total[i] > 0.0 { v / total[i] * 100.0 } else { 0.0 };
                }
                let acc = if v >= 0.0 { &mut pos[i] } else { &mut neg[i] };
                let base = *acc;
                *acc += v;
                (base, *acc)
            }).collect()).collect()
        });
        let secundaria = |i: usize| pilhas.is_none() && e.serie(i).is_some_and(|s| s.eixo_secundario);
        let tem_y2 = (0..self.series.len()).any(|s| ligada(s) && secundaria(s));
        let faixa = |sec: bool| {
            if let Some(p) = &pilhas {
                return p.iter().flatten().fold((0.0f64, 0.0f64), |(a, b), &(x, y)| (a.min(x).min(y), b.max(x).max(y)));
            }
            let vals = self.series.iter().enumerate().filter(|(i, _)| ligada(*i) && secundaria(*i) == sec).flat_map(|(_, s)| s.valores.iter().copied());
            vals.fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), v| (a.min(v), b.max(v)))
        };
        let (a, b) = faixa(false);
        let (a, b) = if a.is_finite() { (a, b) } else { (0.0, 1.0) };
        let incluir_zero = !dispersao;
        let ey = Escala::nova(a, b, &e.eixo_y, incluir_zero);
        let ey2 = tem_y2.then(|| {
            let (a, b) = faixa(true);
            Escala::nova(a, b, &e.eixo_y2, incluir_zero)
        });
        let xs: Vec<f64> = if dispersao {
            self.categorias.iter().map(|c| c.parse::<f64>().unwrap_or(0.0)).collect()
        } else {
            Vec::new()
        };
        let ex = dispersao.then(|| {
            let (a, b) = xs.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), v| (a.min(*v), b.max(*v)));
            Escala::nova(a, b, &e.eixo_x, false)
        });

        let f_eixo_y = Fonte::de(&r.base, &e.eixo_y.rotulos, 1.0);
        let f_eixo_y2 = Fonte::de(&r.base, &e.eixo_y2.rotulos, 1.0);
        let f_eixo_x = Fonte::de(&r.base, &e.eixo_x.rotulos, 1.0);
        let f_tit_y = Fonte::de(&r.base, &e.eixo_y.fonte_titulo, 1.0);
        let f_tit_y2 = Fonte::de(&r.base, &e.eixo_y2.fonte_titulo, 1.0);
        let f_tit_x = Fonte::de(&r.base, &e.eixo_x.fonte_titulo, 1.0);
        let vis_y = e.eixo_y.visivel.unwrap_or(true);
        let vis_y2 = e.eixo_y2.visivel.unwrap_or(true);
        let vis_x = e.eixo_x.visivel.unwrap_or(true);
        let fmt_y = e.eixo_y.formato.unwrap_or_default();
        let fmt_y2 = e.eixo_y2.formato.unwrap_or_default();
        let fmt_x = e.eixo_x.formato.unwrap_or_default();
        let rot_y: Vec<String> = ey.ticks.iter().map(|v| formatar(*v, fmt_y, ey.casas)).collect();
        let rot_y2: Vec<String> = ey2.as_ref().map_or_else(Vec::new, |s| s.ticks.iter().map(|v| formatar(*v, fmt_y2, s.casas)).collect());
        let rot_x_num: Vec<String> = ex.as_ref().map_or_else(Vec::new, |s| s.ticks.iter().map(|v| formatar(*v, fmt_x, s.casas)).collect());
        let giro_x = e.eixo_x.rotacao.unwrap_or(0.0).clamp(-90.0, 90.0);
        let giro_y = e.eixo_y.rotacao.unwrap_or(0.0).clamp(-90.0, 90.0);

        // Reservas em volta da área de plotagem.
        let larg = |v: &[String], f: &Fonte| v.iter().map(|s| largura_texto(s, f.tamanho)).fold(0.0, f32::max);
        let alt_girado = |lw: f32, f: &Fonte, g: f32| {
            let rad = g.to_radians();
            (lw * rad.sin().abs() + f.tamanho * rad.cos().abs()).max(f.tamanho)
        };
        let cats: Vec<String> = if dispersao { rot_x_num.clone() } else { self.categorias.clone() };
        // Eixo das categorias (embaixo; à esquerda nas barras) e de valores.
        let (rot_baixo, f_baixo, giro_baixo, vis_baixo, rot_esq, f_esq, vis_esq) = if horizontal {
            (rot_y.clone(), f_eixo_y.clone(), giro_y, vis_y, cats.clone(), f_eixo_x.clone(), vis_x)
        } else {
            (cats.clone(), f_eixo_x.clone(), giro_x, vis_x, rot_y.clone(), f_eixo_y.clone(), vis_y)
        };
        let lw_baixo = larg(&rot_baixo, &f_baixo).min(caixa.w * 0.4);
        let mut inf = if vis_baixo { alt_girado(lw_baixo, &f_baixo, giro_baixo) + 8.0 } else { 0.0 };
        let mut esq = if vis_esq { larg(&rot_esq, &f_esq).min(caixa.w * 0.35) + 8.0 } else { 0.0 };
        let mut dir = if tem_y2 && vis_y2 && !horizontal { larg(&rot_y2, &f_eixo_y2) + 8.0 } else { lw_baixo.min(60.0) / 2.0 };
        // O primeiro rótulo das categorias fica centrado no começo do eixo:
        // sem reserva à esquerda, nome comprido sai cortado ("na Beatriz").
        if !horizontal && vis_baixo && giro_baixo.abs() < 0.01 {
            let primeiro = rot_baixo.first().map_or(0.0, |t| largura_texto(t, f_baixo.tamanho)).min(caixa.w * 0.3);
            let banda_aprox = if barras && n > 0 { (caixa.w - esq - dir) / n as f32 } else { 0.0 };
            esq = esq.max(primeiro / 2.0 - banda_aprox / 2.0 + 2.0);
        }
        let mut sup = f_eixo_y.tamanho * 0.6;
        let (tit_baixo, f_tb, tit_esq, f_te) = if horizontal {
            (e.eixo_y.titulo.clone(), f_tit_y.clone(), e.eixo_x.titulo.clone(), f_tit_x.clone())
        } else {
            (e.eixo_x.titulo.clone(), f_tit_x.clone(), e.eixo_y.titulo.clone(), f_tit_y.clone())
        };
        let tit_baixo = tit_baixo.filter(|t| !t.trim().is_empty());
        let tit_esq = tit_esq.filter(|t| !t.trim().is_empty());
        let tit_dir = e.eixo_y2.titulo.clone().filter(|t| !t.trim().is_empty() && tem_y2 && !horizontal);
        if tit_baixo.is_some() {
            inf += f_tb.tamanho * 1.4;
        }
        if tit_esq.is_some() {
            esq += f_te.tamanho * 1.4;
        }
        if tit_dir.is_some() {
            dir += f_tit_y2.tamanho * 1.4;
        }
        let unidade = e.eixo_y.unidade.clone().filter(|u| !u.trim().is_empty());
        if unidade.is_some() {
            sup = sup.max(f_eixo_y.tamanho * 1.4);
        }
        let plot = caixa.encolher(sup, inf, esq, dir);
        let mp = &e.margens_plotagem;
        let dados = plot.encolher(mp.superior.unwrap_or(0.0), mp.inferior.unwrap_or(0.0), mp.esquerda.unwrap_or(0.0), mp.direita.unwrap_or(0.0));

        // Fundo da área de plotagem.
        let b = r.linha(&e.area_plotagem.borda, false, r.eixo_cor(), 1.0);
        d.area(plot, "plotagem", e.area_plotagem.preenchimento.as_ref(), r.tema.and_then(|t| t.plotagem).map(|c| c.0),
            b, e.area_plotagem.sombra.as_ref(), e.area_plotagem.raio.unwrap_or(0.0));

        // Mapas.
        let valor_px = |esc: &Escala, v: f64| -> f32 {
            let f = esc.fracao(v);
            if horizontal { dados.x + dados.w * f } else { dados.y + dados.h * (1.0 - f) }
        };
        let centro_cat = barras;
        let cat_frac = |i: usize| -> f32 {
            let t = if centro_cat { (i as f32 + 0.5) / n as f32 } else if n == 1 { 0.5 } else { i as f32 / (n - 1) as f32 };
            if e.eixo_x.invertido { 1.0 - t } else { t }
        };
        let cat_px = |i: usize| -> f32 {
            if let (Some(ex), true) = (ex.as_ref(), dispersao) {
                return dados.x + dados.w * ex.fracao(xs[i]);
            }
            let t = cat_frac(i);
            if horizontal { dados.y + dados.h * t } else { dados.x + dados.w * t }
        };

        // Grades.
        let cor_grade = r.tema.map_or(GRADE_PADRAO, |t| t.grade.0);
        let grade_vis = r.tema.is_some_and(|t| t.grade_visivel);
        let grade_valores = |d: &mut Desenho, esc: &Escala, eixo: &Eixo, classe: &str| {
            for (lista, l, vis, cor, esp) in [
                (&esc.menores, &eixo.grade_secundaria, false, com_alfa(cor_grade, 0.6), 0.6),
                (&esc.ticks, &eixo.grade_principal, grade_vis, cor_grade, 1.0),
            ] {
                if let Some((c, e, t)) = r.linha(l, vis, cor, esp) {
                    for v in lista {
                        let p = valor_px(esc, *v);
                        if horizontal {
                            d.linha(p, plot.y, p, plot.y + plot.h, c, e, t, classe);
                        } else {
                            d.linha(plot.x, p, plot.x + plot.w, p, c, e, t, classe);
                        }
                    }
                }
            }
        };
        grade_valores(d, &ey, &e.eixo_y, "grade-y");
        if let Some(ex) = &ex {
            // Dispersão: grade vertical do X numérico.
            for (lista, l, vis, cor, esp) in [
                (&ex.menores, &e.eixo_x.grade_secundaria, false, com_alfa(cor_grade, 0.6), 0.6),
                (&ex.ticks, &e.eixo_x.grade_principal, false, cor_grade, 1.0),
            ] {
                if let Some((c, es, t)) = r.linha(l, vis, cor, esp) {
                    for v in lista {
                        let p = dados.x + dados.w * ex.fracao(*v);
                        d.linha(p, plot.y, p, plot.y + plot.h, c, es, t, "grade-x");
                    }
                }
            }
        } else {
            if let Some((c, es, t)) = r.linha(&e.eixo_x.grade_principal, false, cor_grade, 1.0) {
                for i in 0..n {
                    let p = cat_px(i);
                    if horizontal { d.linha(plot.x, p, plot.x + plot.w, p, c, es, t, "grade-x") } else { d.linha(p, plot.y, p, plot.y + plot.h, c, es, t, "grade-x") }
                }
            }
            if let Some((c, es, t)) = r.linha(&e.eixo_x.grade_secundaria, false, com_alfa(cor_grade, 0.6), 0.6) {
                if centro_cat {
                    for i in 0..=n {
                        let tt = i as f32 / n as f32;
                        let p = if horizontal { dados.y + dados.h * tt } else { dados.x + dados.w * tt };
                        if horizontal { d.linha(plot.x, p, plot.x + plot.w, p, c, es, t, "grade-x") } else { d.linha(p, plot.y, p, plot.y + plot.h, c, es, t, "grade-x") }
                    }
                }
            }
        }

        // Séries (recortadas na área de plotagem).
        let _ = write!(d.defs, "<clipPath id=\"recorte-plotagem\"><rect x=\"{:.2}\" y=\"{:.2}\" width=\"{:.2}\" height=\"{:.2}\"/></clipPath>", plot.x - 1.0, plot.y - 1.0, plot.w + 2.0, plot.h + 2.0);
        d.corpo.push_str("<g clip-path=\"url(#recorte-plotagem)\">\n");
        let escala_da = |i: usize| -> &Escala { if secundaria(i) { ey2.as_ref().unwrap_or(&ey) } else { &ey } };
        let mut rotulos_dados: Vec<(f32, f32, String, Fonte, Ancora)> = Vec::new();
        let ns = (0..self.series.len()).filter(|s| ligada(*s)).count().max(1);
        let raio_barras = e.barras.raio.unwrap_or(r.tema.map_or(0.0, |t| t.raio_barras));
        let banda = if horizontal { dados.h } else { dados.w } / n as f32;
        // Largura de cada barra e o passo entre as barras de um grupo.
        let (largura_barra, passo_barra, grupo) = match (e.barras.espacamento, e.barras.sobreposicao) {
            // Empilhadas: uma barra por categoria, com a largura do grupo.
            (esp, _) if pilhas.is_some() => {
                let g = esp.map_or((banda * 0.6).min(112.0), |g| banda / (1.0 + g.clamp(0.0, 500.0) / 100.0));
                (g, 0.0, g)
            }
            (None, None) => {
                let grupo = (banda * 0.72).min(112.0);
                (grupo / ns as f32 * 0.94, grupo / ns as f32, grupo)
            }
            (g, o) => {
                let g = g.unwrap_or(100.0).clamp(0.0, 500.0) / 100.0;
                let o = o.unwrap_or(0.0).clamp(-100.0, 100.0) / 100.0;
                let bw = banda / (ns as f32 - (ns as f32 - 1.0) * o + g);
                let passo = bw * (1.0 - o);
                (bw, passo, bw + passo * (ns as f32 - 1.0))
            }
        };
        for (s, serie) in self.series.iter().enumerate() {
            if !ligada(s) {
                continue;
            }
            let est = e.serie(s);
            let esc = escala_da(s);
            let cor = r.cor_serie(self, s);
            let rot = r.rotulos_da_serie(s);
            let f_rot = Fonte::de(&r.base, &rot.fonte, 0.92);
            let mostra_rot = rot.visivel.unwrap_or(false);
            let texto_rot = |i: usize, v: f64| -> String {
                let mut p = Vec::new();
                if rot.nome_serie.unwrap_or(false) {
                    p.push(serie.nome.clone());
                }
                if rot.categoria.unwrap_or(false) {
                    p.push(self.categorias[i].clone());
                }
                if rot.valor.unwrap_or(true) {
                    p.push(formatar(v, rot.formato.or(e.eixo_y.formato).unwrap_or_default(), casas_de(v)));
                }
                p.join("; ")
            };
            let contorno = est.and_then(|s| s.contorno).map(|c| c.0);
            let esp_contorno = est.and_then(|s| s.espessura_contorno).unwrap_or(1.0);
            if barras {
                let base_v = esc.base(e.eixo_y.cruzamento.filter(|_| !secundaria(s)).or(e.eixo_y2.cruzamento.filter(|_| secundaria(s))));
                let pb = valor_px(esc, base_v);
                for (i, &v) in serie.valores.iter().enumerate() {
                    let t0 = if horizontal { dados.y } else { dados.x } + banda * if e.eixo_x.invertido { (n - 1 - i) as f32 } else { i as f32 };
                    let pos = t0 + (banda - grupo) / 2.0 + passo_barra * vagas[s] as f32;
                    let (pb, pv) = match &pilhas {
                        Some(p) => (valor_px(esc, p[s][i].0), valor_px(esc, p[s][i].1)),
                        None => (pb, valor_px(esc, v)),
                    };
                    let (x, y, bw, bh) = if horizontal {
                        (pb.min(pv), pos, (pv - pb).abs(), largura_barra)
                    } else {
                        (pos, pb.min(pv), largura_barra, (pv - pb).abs())
                    };
                    let cor_p = r.cor_ponto(self, s, i);
                    let cont = est.and_then(|s| s.pontos.get(&i)).and_then(|p| p.contorno).map(|c| c.0).or(contorno)
                        .map_or_else(String::new, |c| format!(" {}", tracar(c, esp_contorno, Traco::Continuo)));
                    let rx = raio_barras.min(bw / 2.0).min(bh / 2.0).max(0.0);
                    let _ = writeln!(d.corpo, "<rect class=\"barra s{s} p{i}\" x=\"{x:.2}\" y=\"{y:.2}\" width=\"{bw:.2}\" height=\"{bh:.2}\" rx=\"{rx:.2}\" {}{cont}/>", preencher(cor_p));
                    d.mapa.pontos.push(AlvoPonto { serie: s, ponto: i, forma: FormaAlvo::Retangulo { x, y, largura: bw, altura: bh } });
                    if mostra_rot {
                        let t = texto_rot(i, v);
                        let positivo = if horizontal { pv >= pb } else { pv <= pb };
                        let fs = f_rot.tamanho;
                        // Empilhadas: o rótulo fica no meio do pedaço da série.
                        let pos_r = rot.posicao.unwrap_or(if pilhas.is_some() { PosicaoRotulo::Centro } else { PosicaoRotulo::default() });
                        if horizontal {
                            let cy = y + bh / 2.0 + fs * 0.35;
                            let (lx, anc) = match pos_r {
                                PosicaoRotulo::Centro => (x + bw / 2.0, Ancora::Meio),
                                PosicaoRotulo::DentroFim => if positivo { (pv - 3.0, Ancora::Fim) } else { (pv + 3.0, Ancora::Inicio) },
                                PosicaoRotulo::DentroBase => if positivo { (pb + 3.0, Ancora::Inicio) } else { (pb - 3.0, Ancora::Fim) },
                                _ => if positivo { (pv + 3.0, Ancora::Inicio) } else { (pv - 3.0, Ancora::Fim) },
                            };
                            rotulos_dados.push((lx, cy, t, f_rot.clone(), anc));
                        } else {
                            let cx = x + bw / 2.0;
                            let ly = match pos_r {
                                PosicaoRotulo::Centro => y + bh / 2.0 + fs * 0.35,
                                PosicaoRotulo::DentroFim => if positivo { pv + fs * 1.05 } else { pv - 4.0 },
                                PosicaoRotulo::DentroBase => if positivo { pb - 4.0 } else { pb + fs * 1.05 },
                                _ => if positivo { pv - 4.0 } else { pv + fs * 1.05 },
                            };
                            rotulos_dados.push((cx, ly, t, f_rot.clone(), Ancora::Meio));
                        }
                    }
                }
                continue;
            }
            // Linhas, área, degraus e dispersão (empilhadas: o topo da pilha).
            let topo = |i: usize, v: f64| pilhas.as_ref().map_or(v, |p| p[s][i].1);
            let pontos: Vec<(f32, f32)> = serie.valores.iter().enumerate().map(|(i, v)| (cat_px(i), valor_px(esc, topo(i, *v)))).collect();
            let em_degraus = self.tipo == Tipo::Degraus || (e.degraus && matches!(self.tipo, Tipo::Area | Tipo::Linhas));
            let degrau = |pts: &[(f32, f32)]| -> Vec<(f32, f32)> {
                let mut v = Vec::with_capacity(pts.len() * 2);
                for (k, p) in pts.iter().enumerate() {
                    if k > 0 {
                        v.push((p.0, pts[k - 1].1));
                    }
                    v.push(*p);
                }
                v
            };
            let esp = est.and_then(|s| s.espessura_linha).unwrap_or(2.0);
            let traco = est.and_then(|s| s.traco).unwrap_or_default();
            let suave = est.and_then(|s| s.suavizar).unwrap_or(false);
            let pontos_linha: Vec<(f32, f32)> = if em_degraus { degrau(&pontos) } else { pontos.clone() };
            let suave = suave && !em_degraus;
            if self.tipo == Tipo::Area && pontos.len() > 1 {
                let mut dd = caminho(&pontos_linha, suave);
                if let Some(p) = &pilhas {
                    // Empilhada: a faixa entre o topo da série e o da anterior.
                    let base: Vec<(f32, f32)> = (0..pontos.len()).map(|i| (cat_px(i), valor_px(esc, p[s][i].0))).collect();
                    let base = if em_degraus { degrau(&base) } else { base };
                    for q in base.iter().rev() {
                        let _ = write!(dd, " L{:.2} {:.2}", q.0, q.1);
                    }
                    dd.push_str(" Z");
                } else {
                    let pb = valor_px(esc, esc.base(e.eixo_y.cruzamento));
                    let (p0, pn) = (pontos[0], pontos[pontos.len() - 1]);
                    let _ = write!(dd, " L{:.2} {pb:.2} L{:.2} {pb:.2} Z", pn.0, p0.0);
                }
                let opac = if pilhas.is_some() { 0.75 } else { 0.35 };
                let _ = writeln!(d.corpo, "<path class=\"area-serie s{s}\" d=\"{dd}\" {}/>", preencher(com_alfa(cor, opac)));
            }
            if !dispersao && pontos_linha.len() > 1 && esp > 0.0 {
                let pinta_ponto = est.is_some_and(|s| s.pontos.values().any(|p| p.cor.is_some()));
                if pinta_ponto && !em_degraus {
                    // Cada trecho na cor do ponto em que chega.
                    for k in 1..pontos.len() {
                        let c = r.cor_ponto(self, s, k);
                        let seg = if suave && pontos.len() >= 3 {
                            format!("M{:.2} {:.2} {}", pontos[k - 1].0, pontos[k - 1].1, trecho_suave(&pontos, k - 1))
                        } else {
                            format!("M{:.2} {:.2} L{:.2} {:.2}", pontos[k - 1].0, pontos[k - 1].1, pontos[k].0, pontos[k].1)
                        };
                        let _ = writeln!(d.corpo, "<path class=\"linha s{s} trecho{k}\" d=\"{seg}\" fill=\"none\" {} stroke-linejoin=\"round\" stroke-linecap=\"round\"/>", tracar(c, esp, traco));
                    }
                } else {
                    let _ = writeln!(d.corpo, "<path class=\"linha s{s}\" d=\"{}\" fill=\"none\" {} stroke-linejoin=\"round\" stroke-linecap=\"round\"/>", caminho(&pontos_linha, suave), tracar(cor, esp, traco));
                }
            }
            let forma = est.and_then(|s| s.marcador).unwrap_or(if dispersao { FormaMarcador::Quadrado } else { FormaMarcador::Nenhum });
            let tam = est.and_then(|s| s.tamanho_marcador).unwrap_or(if dispersao { 5.0 } else { 6.0 });
            let tem_ponto = est.is_some_and(|s| !s.pontos.is_empty());
            for (i, p) in pontos.iter().enumerate() {
                let forma_i = if forma == FormaMarcador::Nenhum && tem_ponto && est.is_some_and(|s| s.pontos.get(&i).is_some_and(|q| q.cor.is_some())) {
                    // Ponto pintado numa linha sem marcador: o marcador aparece nele.
                    FormaMarcador::Circulo
                } else {
                    forma
                };
                let cont = est.and_then(|s| s.pontos.get(&i)).and_then(|q| q.contorno).map(|c| c.0).or(contorno).map(|c| (c, esp_contorno));
                marcador(d, forma_i, p.0, p.1, tam, r.cor_ponto(self, s, i), cont);
                d.mapa.pontos.push(AlvoPonto { serie: s, ponto: i, forma: FormaAlvo::Circulo { x: p.0, y: p.1, raio: (tam / 2.0).max(6.0) } });
                if mostra_rot {
                    let fs = f_rot.tamanho;
                    let (lx, ly, anc) = match rot.posicao.unwrap_or_default() {
                        PosicaoRotulo::Abaixo => (p.0, p.1 + fs * 1.25, Ancora::Meio),
                        PosicaoRotulo::Esquerda => (p.0 - tam, p.1 + fs * 0.35, Ancora::Fim),
                        PosicaoRotulo::Direita => (p.0 + tam, p.1 + fs * 0.35, Ancora::Inicio),
                        PosicaoRotulo::Centro => (p.0, p.1 + fs * 0.35, Ancora::Meio),
                        _ => (p.0, p.1 - tam.max(4.0) - 2.0, Ancora::Meio),
                    };
                    rotulos_dados.push((lx, ly, texto_rot(i, serie.valores[i]), f_rot.clone(), anc));
                }
            }
        }
        // Elementos analíticos (tendência, barras de erro, máx/mín/média),
        // por cima das séries e dentro do recorte da plotagem.
        let mut legendas_analise: Vec<(String, Cor)> = Vec::new();
        let fundo_texto = [&e.area_plotagem.preenchimento, &e.area_grafico.preenchimento].into_iter()
            .find_map(|p| match p { Some(Preenchimento::Solido { cor }) if cor.0.a == 255 => Some(cor.0), _ => None })
            .or_else(|| r.tema.and_then(|t| t.plotagem).map(|c| c.0))
            .or_else(|| r.tema.map(|t| t.fundo.0))
            .unwrap_or(Cor::rgba(255, 255, 255, 255));
        d.halo = Some(fundo_texto);
        for (s, serie) in self.series.iter().enumerate() {
            let Some(an) = e.serie(s).map(|x| &x.analise).filter(|a| !a.vazia()) else { continue };
            if !ligada(s) || serie.valores.is_empty() {
                continue;
            }
            let esc = escala_da(s);
            let cor = r.cor_serie(self, s);
            // Valor de cada ponto (o topo da pilha nas empilhadas).
            let ys: Vec<f64> = match &pilhas {
                Some(p) => p[s].iter().map(|q| q.1).collect(),
                None => serie.valores.clone(),
            };
            let xs: Vec<f64> = if dispersao { xs.clone() } else { (1..=ys.len()).map(|i| i as f64).collect() };
            // Barras: o centro da barra desta série dentro do grupo.
            let desvio = if barras { passo_barra * vagas[s] as f32 + largura_barra / 2.0 - grupo / 2.0 } else { 0.0 };
            let ponto = |xv: f64, yv: f64| -> (f32, f32) {
                let cat = if let (Some(ex), true) = (ex.as_ref(), dispersao) {
                    dados.x + dados.w * ex.fracao(xv)
                } else {
                    let t = if centro_cat { ((xv - 0.5) / n as f64) as f32 } else if n == 1 { 0.5 } else { ((xv - 1.0) / (n - 1) as f64) as f32 };
                    let t = if e.eixo_x.invertido { 1.0 - t } else { t };
                    (if horizontal { dados.y + dados.h * t } else { dados.x + dados.w * t }) + desvio
                };
                let v = valor_px(esc, yv);
                if horizontal { (v, cat) } else { (cat, v) }
            };
            // Linhas de máximo, mínimo e média, de lado a lado da plotagem.
            let f_ref = Fonte::de(&r.base, &Texto::default(), 0.85);
            let finitos = || ys.iter().copied().filter(|v| v.is_finite());
            let n_fin = finitos().count();
            if n_fin > 0 {
                let refs = [
                    (an.maximo, "Máx", finitos().fold(f64::NEG_INFINITY, f64::max), Traco::Tracejado),
                    (an.minimo, "Mín", finitos().fold(f64::INFINITY, f64::min), Traco::Tracejado),
                    (an.media, "Média", finitos().sum::<f64>() / n_fin as f64, Traco::Pontilhado),
                ];
                for (ligado, nome, v, traco) in refs {
                    if !ligado {
                        continue;
                    }
                    let p = valor_px(esc, v);
                    let rotulo = format!("{nome}: {}", formatar(v, e.eixo_y.formato.unwrap_or_default(), casas_de(v)));
                    let mut fr = f_ref.clone();
                    fr.cor = cor;
                    if horizontal {
                        d.linha(p, plot.y, p, plot.y + plot.h, cor, 1.2, traco, "analise-ref");
                        d.texto(p + 3.0, plot.y + fr.tamanho, &rotulo, &fr, Ancora::Inicio, 0.0, "analise-ref-rotulo");
                    } else {
                        d.linha(plot.x, p, plot.x + plot.w, p, cor, 1.2, traco, "analise-ref");
                        d.texto(plot.x + plot.w - 3.0, p - 3.0, &rotulo, &fr, Ancora::Fim, 0.0, "analise-ref-rotulo");
                    }
                }
            }
            // Barras de erro.
            if let Some(b) = &an.erro {
                let c = b.cor.map_or(EIXO_PADRAO, |c| c.0);
                let esp = b.espessura.unwrap_or(1.2);
                let ponta = if b.sem_ponta { 0.0 } else { (largura_barra * 0.25).clamp(3.0, 6.0) };
                for (i, (baixo, cima)) in crate::analitico::extensao_erro(b, &ys).into_iter().enumerate() {
                    if !ys[i].is_finite() {
                        continue;
                    }
                    let (a, z) = (ponto(xs[i], baixo), ponto(xs[i], cima));
                    d.linha(a.0, a.1, z.0, z.1, c, esp, Traco::Continuo, "analise-erro");
                    if ponta > 0.0 {
                        for q in [a, z] {
                            if horizontal {
                                d.linha(q.0, q.1 - ponta, q.0, q.1 + ponta, c, esp, Traco::Continuo, "analise-erro");
                            } else {
                                d.linha(q.0 - ponta, q.1, q.0 + ponta, q.1, c, esp, Traco::Continuo, "analise-erro");
                            }
                        }
                    }
                }
            }
            // Linha de tendência.
            if let Some(t) = &an.tendencia {
                let c = t.cor.map_or(cor, |c| c.0);
                let esp = t.espessura.unwrap_or(1.6);
                let traco = t.traco.unwrap_or(Traco::Tracejado);
                let pts: Vec<(f32, f32)> = if t.tipo == crate::analitico::TipoTendencia::MediaMovel {
                    crate::analitico::media_movel(&xs, &ys, t.periodo).into_iter().map(|(x, y)| ponto(x, y)).collect()
                } else if let Some(aj) = crate::analitico::ajustar(t, &xs, &ys) {
                    let (mut x0, mut x1) = xs.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), v| (a.min(*v), b.max(*v)));
                    x0 -= t.recuar.max(0.0);
                    x1 += t.avancar.max(0.0);
                    if matches!(aj, crate::analitico::Ajuste::Logaritmica { .. } | crate::analitico::Ajuste::Potencia { .. }) {
                        x0 = x0.max(1e-9);
                    }
                    let passos = if matches!(aj, crate::analitico::Ajuste::Linear { .. }) { 1 } else { 80 };
                    let mut texto = Vec::new();
                    if t.mostrar_equacao {
                        texto.push(aj.equacao());
                    }
                    if t.mostrar_r2 {
                        texto.push(format!("R² = {}", format!("{:.4}", aj.r2()).replace('.', ",")));
                    }
                    if !texto.is_empty() {
                        legendas_analise.push((texto.join("   "), c));
                    }
                    (0..=passos).map(|k| x0 + (x1 - x0) * k as f64 / passos as f64)
                        .map(|x| (x, aj.valor(x))).filter(|(_, y)| y.is_finite())
                        .map(|(x, y)| ponto(x, y)).collect()
                } else {
                    Vec::new()
                };
                if pts.len() > 1 {
                    let _ = writeln!(d.corpo, "<path class=\"tendencia s{s}\" d=\"{}\" fill=\"none\" {} stroke-linejoin=\"round\"/>", caminho(&pts, false), tracar(c, esp, traco));
                }
            }
        }
        // Equação e R² no canto de cima da plotagem, uma linha por série.
        let f_eq = Fonte::de(&r.base, &Texto::default(), 0.9);
        for (k, (t, c)) in legendas_analise.iter().enumerate() {
            let mut f = f_eq.clone();
            f.cor = *c;
            d.texto(plot.x + 6.0, plot.y + f.tamanho * (1.2 + 1.3 * k as f32), t, &f, Ancora::Inicio, 0.0, "tendencia-equacao");
        }
        d.halo = None;
        d.corpo.push_str("</g>\n");

        // Linhas dos eixos e marcas.
        let cor_eixo = r.eixo_cor();
        // Linha das categorias: no cruzamento do eixo de valores.
        let base_cat = valor_px(&ey, ey.base(e.eixo_y.cruzamento));
        // Linha dos valores: no cruzamento do eixo das categorias.
        let base_val = match (e.eixo_x.cruzamento, ex.as_ref()) {
            (Some(v), Some(ex)) => dados.x + dados.w * ex.fracao(v),
            (Some(v), None) if n > 0 => {
                let i = ((v.round() as i64 - 1).clamp(0, n as i64 - 1)) as usize;
                let t = if centro_cat { (i as f32) / n as f32 } else { cat_frac(i) };
                let t = if centro_cat && e.eixo_x.invertido { 1.0 - t } else { t };
                if horizontal { dados.y + dados.h * t } else { dados.x + dados.w * t }
            }
            _ => {
                let x0 = if ex.is_some() { dados.x + dados.w * ex.as_ref().map_or(0.0, |s| s.fracao(s.base(None))) } else if horizontal { plot.y + plot.h } else { plot.x };
                if ex.is_some() { x0.clamp(plot.x, plot.x + plot.w) } else { x0 }
            }
        };
        if let Some((c, es, t)) = r.linha(&e.eixo_x.linha, vis_x || dispersao, cor_eixo, 1.0) {
            if horizontal { d.linha(base_cat, plot.y, base_cat, plot.y + plot.h, c, es, t, "eixo-x") } else { d.linha(plot.x, base_cat, plot.x + plot.w, base_cat, c, es, t, "eixo-x") }
        }
        if let Some((c, es, t)) = r.linha(&e.eixo_y.linha, vis_y, cor_eixo, 1.0) {
            if horizontal { d.linha(plot.x, base_val, plot.x + plot.w, base_val, c, es, t, "eixo-y") } else { d.linha(base_val, plot.y, base_val, plot.y + plot.h, c, es, t, "eixo-y") }
        }
        if let (Some(e2), true) = (&ey2, !horizontal) {
            if let Some((c, es, t)) = r.linha(&e.eixo_y2.linha, vis_y2, cor_eixo, 1.0) {
                d.linha(plot.x + plot.w, plot.y, plot.x + plot.w, plot.y + plot.h, c, es, t, "eixo-y2");
            }
            if vis_y2 {
                for (v, s) in e2.ticks.iter().zip(&rot_y2) {
                    let p = valor_px(e2, *v);
                    d.texto(plot.x + plot.w + 5.0, p + f_eixo_y2.tamanho * 0.35, s, &f_eixo_y2, Ancora::Inicio, 0.0, "rotulo-y2");
                }
            }
        }

        // Rótulos dos eixos.
        let borda_baixo = plot.y + plot.h;
        let rotulo_baixo = |d: &mut Desenho, x: f32, t: &str, f: &Fonte, giro: f32, classe: &str| {
            let y = borda_baixo + 4.0 + f.tamanho * 0.95;
            if giro.abs() > 0.01 {
                d.texto(x, borda_baixo + 6.0, t, f, if giro < 0.0 { Ancora::Fim } else { Ancora::Inicio }, giro, classe);
            } else {
                d.texto(x, y, t, f, Ancora::Meio, 0.0, classe);
            }
        };
        if horizontal {
            if vis_y {
                for (v, s) in ey.ticks.iter().zip(&rot_y) {
                    rotulo_baixo(d, valor_px(&ey, *v), s, &f_eixo_y, giro_y, "rotulo-y");
                }
            }
            if vis_x {
                let max_w = esq - 8.0 - if tit_esq.is_some() { f_te.tamanho * 1.4 } else { 0.0 };
                for (i, c) in self.categorias.iter().enumerate() {
                    d.texto(plot.x - 6.0, cat_px(i) + f_eixo_x.tamanho * 0.35, &cortar(c, max_w, f_eixo_x.tamanho), &f_eixo_x, Ancora::Fim, 0.0, "rotulo-x");
                }
            }
        } else {
            if vis_y {
                let max_w = esq - 8.0 - if tit_esq.is_some() { f_te.tamanho * 1.4 } else { 0.0 };
                for (v, s) in ey.ticks.iter().zip(&rot_y) {
                    let p = valor_px(&ey, *v);
                    d.texto(plot.x - 6.0, p + f_eixo_y.tamanho * 0.35, &cortar(s, max_w + 4.0, f_eixo_y.tamanho), &f_eixo_y, Ancora::Fim, 0.0, "rotulo-y");
                }
            }
            if vis_x {
                if let Some(exx) = &ex {
                    for (v, s) in exx.ticks.iter().zip(&rot_x_num) {
                        rotulo_baixo(d, dados.x + dados.w * exx.fracao(*v), s, &f_eixo_x, giro_x, "rotulo-x");
                    }
                } else if n > 0 {
                    // Rótulos ralos quando não cabem.
                    let cabe = if giro_x.abs() > 0.01 { f_eixo_x.tamanho * 1.3 } else { (lw_baixo + 8.0).max(24.0) };
                    let espaco = dados.w / n as f32;
                    let passo = ((cabe / espaco).ceil() as usize).max(1);
                    let lim = if giro_x.abs() > 0.01 { lw_baixo } else { espaco * passo as f32 - 4.0 };
                    for i in (0..n).step_by(passo) {
                        rotulo_baixo(d, cat_px(i), &cortar(&self.categorias[i], lim, f_eixo_x.tamanho), &f_eixo_x, giro_x, "rotulo-x");
                    }
                }
            }
        }
        if let Some(t) = &tit_baixo {
            d.texto(plot.x + plot.w / 2.0, caixa.y + caixa.h - f_tb.tamanho * 0.25, t, &f_tb, Ancora::Meio, 0.0, "titulo-eixo");
        }
        if let Some(t) = &tit_esq {
            let (x, y) = (caixa.x + f_te.tamanho * 0.95, plot.y + plot.h / 2.0);
            d.texto(x, y, t, &f_te, Ancora::Meio, -90.0, "titulo-eixo");
        }
        if let Some(t) = &tit_dir {
            let (x, y) = (caixa.x + caixa.w - f_tit_y2.tamanho * 0.35, plot.y + plot.h / 2.0);
            d.texto(x, y, t, &f_tit_y2, Ancora::Meio, -90.0, "titulo-eixo");
        }
        if let Some(u) = &unidade {
            d.texto(plot.x + plot.w, plot.y - 4.0, u, &f_eixo_y, Ancora::Fim, 0.0, "unidade");
        }
        for (x, y, t, f, a) in rotulos_dados {
            d.texto(x, y, &t, &f, a, 0.0, "rotulo");
        }
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::Serie;

    fn g(tipo: Tipo) -> Grafico {
        Grafico {
            titulo: "Vendas".into(),
            tipo,
            categorias: vec!["Jan".into(), "Fev".into(), "Mar".into()],
            series: vec![
                Serie { nome: "Loja A".into(), valores: vec![10.0, 20.0, 30.0], cor: Cor::rgb(66, 133, 244) },
                Serie { nome: "Loja B".into(), valores: vec![5.0, 15.0, 25.0], cor: Cor::rgb(234, 67, 53) },
            ],
        }
    }
    fn atributo(svg: &str, classe: &str, nome: &str) -> Vec<f32> {
        svg.lines().filter(|l| l.contains(&format!("class=\"{classe}"))).filter_map(|l| {
            let k = format!(" {nome}=\"");
            let i = l.find(&k)? + k.len();
            l[i..].split('"').next()?.parse().ok()
        }).collect()
    }

    #[test]
    fn colunas_empilhadas_e_cem_por_cento() {
        let mut gr = g(Tipo::Colunas);
        gr.series.push(gr.series[0].clone());
        let mut e = EstiloGrafico { empilhar: Empilhar::Empilhado, ..Default::default() };
        let svg = gr.exportar_svg_formatado(600.0, 300.0, &e).unwrap();
        // Mesma categoria: as duas séries com o mesmo x e uma em cima da outra.
        let xs = atributo(&svg, "barra s0 p0", "x");
        let xs1 = atributo(&svg, "barra s1 p0", "x");
        assert_eq!(xs, xs1);
        let (y0, h0) = (atributo(&svg, "barra s0 p0", "y")[0], atributo(&svg, "barra s0 p0", "height")[0]);
        let y1 = atributo(&svg, "barra s1 p0", "y")[0];
        assert!((y1 + atributo(&svg, "barra s1 p0", "height")[0] - y0).abs() < 0.05, "s1 encosta em cima de s0");
        assert!(h0 > 0.0);
        e.empilhar = Empilhar::Cem;
        let svg = gr.exportar_svg_formatado(600.0, 300.0, &e).unwrap();
        // 10, 5 e 10 em 100%: 40%, 20% e 40% da mesma altura total.
        let h: Vec<f32> = (0..3).map(|s| atributo(&svg, &format!("barra s{s} p0"), "height")[0]).collect();
        assert!((h[0] - h[2]).abs() < 0.05 && (h[0] - 2.0 * h[1]).abs() < 0.05, "{h:?}");
        // O fundo do gráfico continua lá.
        assert!(svg.contains("area-grafico") || svg.contains("<rect"));
    }

    #[test]
    fn padrao_desenha_os_sete_tipos_e_e_deterministico() {
        for t in [Tipo::Colunas, Tipo::Barras, Tipo::Linhas, Tipo::Area, Tipo::Degraus, Tipo::Dispersao] {
            let mut gr = g(t);
            if t == Tipo::Dispersao {
                gr.categorias = vec!["1".into(), "2".into(), "4".into()];
            }
            let svg = gr.exportar_svg_formatado(640.0, 360.0, &EstiloGrafico::default()).unwrap();
            assert!(svg.starts_with("<svg") && svg.ends_with("</svg>\n"));
            assert!(svg.contains("Vendas") && svg.contains("Loja B"), "{t:?}");
            assert!(!svg.contains("grade-y"), "sem tema não tem grade (aparência de sempre)");
            assert_eq!(svg, gr.exportar_svg_formatado(640.0, 360.0, &EstiloGrafico::default()).unwrap());
        }
        let mut p = g(Tipo::Pizza);
        p.series.truncate(1);
        let svg = p.exportar_svg_formatado(400.0, 300.0, &EstiloGrafico::default()).unwrap();
        assert_eq!(svg.matches("class=\"fatia\"").count(), 3);
        assert!(svg.contains(">Jan</text>"));
    }

    #[test]
    fn tema_cor_por_ponto_e_espacamento_das_barras() {
        let mut e = EstiloGrafico { tema: Some("floresta".into()), ..Default::default() };
        let svg = g(Tipo::Colunas).exportar_svg_formatado(600.0, 300.0, &e).unwrap();
        assert!(svg.contains("#2d6a4f") && svg.contains("#f4f8f2"), "paleta e fundo do tema");
        assert!(svg.contains("grade-y"));
        e.serie_mut(0).pontos.insert(1, EstiloPonto { cor: Some(CorEstilo::hex(0xff0000)), ..Default::default() });
        let svg = g(Tipo::Colunas).exportar_svg_formatado(600.0, 300.0, &e).unwrap();
        let barra = svg.lines().find(|l| l.contains("class=\"barra s0 p1\"")).unwrap();
        assert!(barra.contains("#ff0000"));
        let outra = svg.lines().find(|l| l.contains("class=\"barra s0 p0\"")).unwrap();
        assert!(!outra.contains("#ff0000"), "só um ponto pintado");
        let larg0 = atributo(&svg, "barra s0 p0", "width")[0];
        e.barras.espacamento = Some(0.0);
        e.barras.sobreposicao = Some(0.0);
        let svg = g(Tipo::Colunas).exportar_svg_formatado(600.0, 300.0, &e).unwrap();
        let larg1 = atributo(&svg, "barra s0 p0", "width")[0];
        assert!(larg1 > larg0 * 1.2, "sem espaço as barras engordam: {larg0} → {larg1}");
        e.barras.sobreposicao = Some(100.0);
        let svg = g(Tipo::Colunas).exportar_svg_formatado(600.0, 300.0, &e).unwrap();
        let x0 = atributo(&svg, "barra s0 p0", "x")[0];
        let x1 = atributo(&svg, "barra s1 p0", "x")[0];
        assert!((x0 - x1).abs() < 0.01, "sobreposição total: mesma posição");
    }

    #[test]
    fn margens_degrade_sombra_e_borda_da_area() {
        let mut e = EstiloGrafico::default();
        let antes = atributo(&g(Tipo::Colunas).exportar_svg_formatado(600.0, 300.0, &e).unwrap(), "area-plotagem", "x")[0];
        e.margens_grafico.esquerda = Some(80.0);
        e.area_grafico.preenchimento = Some(Preenchimento::Degrade { inicio: CorEstilo::hex(0x000000), fim: CorEstilo::hex(0x2f5597), angulo: 90.0 });
        e.area_grafico.sombra = Some(Sombra::default());
        e.area_grafico.borda = Linha { visivel: Some(true), cor: Some(CorEstilo::hex(0x123456)), espessura: Some(3.0), traco: Some(Traco::Tracejado) };
        let svg = g(Tipo::Colunas).exportar_svg_formatado(600.0, 300.0, &e).unwrap();
        let depois = atributo(&svg, "area-plotagem", "x")[0];
        assert!((depois - antes - 70.0).abs() < 0.5, "margem esquerda empurra a plotagem: {antes} → {depois}");
        assert!(svg.contains("linearGradient id=\"grad-grafico\"") && svg.contains("url(#grad-grafico)"));
        assert!(svg.contains("feDropShadow") && svg.contains("url(#sombra-grafico)"));
        assert!(svg.contains("stroke=\"#123456\" stroke-width=\"3.00\" stroke-dasharray=\"12.0 9.0\""));
    }

    #[test]
    fn eixos_minimo_maximo_unidade_log_invertido_formato_e_rotacao() {
        let mut e = EstiloGrafico::default();
        e.eixo_y.minimo = Some(0.0);
        e.eixo_y.maximo = Some(40.0);
        e.eixo_y.unidade_principal = Some(10.0);
        e.eixo_y.formato = Some(FormatoNum::Moeda { casas: 2 });
        let svg = g(Tipo::Linhas).exportar_svg_formatado(600.0, 300.0, &e).unwrap();
        for v in ["R$ 0,00", "R$ 10,00", "R$ 40,00"] {
            assert!(svg.contains(&format!(">{v}</text>")), "{v}");
        }
        assert_eq!(svg.matches("class=\"rotulo-y\"").count(), 5);
        e.eixo_y = Eixo { logaritmica: true, ..Default::default() };
        let mut gr = g(Tipo::Linhas);
        gr.series[0].valores = vec![1.0, 100.0, 1000.0];
        let svg = gr.exportar_svg_formatado(600.0, 300.0, &e).unwrap();
        for v in [">1</text>", ">10</text>", ">100</text>", ">1.000</text>"] {
            assert!(svg.contains(v), "{v}");
        }
        // Invertido: o maior valor fica em cima → embaixo.
        let ys = |svg: &str| atributo(svg, "rotulo-y", "y");
        e.eixo_y = Eixo::default();
        let normal = ys(&g(Tipo::Colunas).exportar_svg_formatado(600.0, 300.0, &e).unwrap());
        e.eixo_y.invertido = true;
        let inv = ys(&g(Tipo::Colunas).exportar_svg_formatado(600.0, 300.0, &e).unwrap());
        assert!(normal.first() > normal.last() && inv.first() < inv.last());
        e.eixo_x.rotacao = Some(-45.0);
        e.eixo_x.titulo = Some("Mês".into());
        let svg = g(Tipo::Colunas).exportar_svg_formatado(600.0, 300.0, &e).unwrap();
        assert!(svg.contains("rotate(-45.0"));
        assert!(svg.contains(">Mês</text>"));
    }

    #[test]
    fn grades_eixo_secundario_suavizar_marcador_e_rotulos() {
        let mut e = EstiloGrafico::default();
        e.eixo_y.grade_principal = Linha { visivel: Some(true), cor: Some(CorEstilo::hex(0xabcdef)), espessura: Some(2.0), traco: Some(Traco::Pontilhado) };
        e.eixo_y.grade_secundaria.visivel = Some(true);
        let s = e.serie_mut(1);
        s.eixo_secundario = true;
        s.suavizar = Some(true);
        s.marcador = Some(FormaMarcador::Losango);
        s.rotulos.visivel = Some(true);
        s.rotulos.formato = Some(FormatoNum::Decimal { casas: 1 });
        e.eixo_y2.titulo = Some("Segundo".into());
        let svg = g(Tipo::Linhas).exportar_svg_formatado(700.0, 340.0, &e).unwrap();
        assert!(svg.contains("stroke=\"#abcdef\" stroke-width=\"2.00\" stroke-dasharray"));
        assert!(svg.matches("class=\"grade-y\"").count() > 10, "principal + secundária");
        assert!(svg.contains("class=\"rotulo-y2\"") && svg.contains(">Segundo</text>"));
        let linha_b = svg.lines().find(|l| l.contains("class=\"linha s1\"")).unwrap();
        assert!(linha_b.contains(" C"), "suavizada");
        assert!(svg.contains("class=\"marcador\" d=\"M"));
        assert!(svg.contains(">25,0</text>"), "rótulo com formato da série");
        assert!(!svg.contains(">30,0</text>"), "série A sem rótulos");
    }

    #[test]
    fn clique_na_legenda_desliga_a_serie_e_mantem_o_item_hachurado() {
        let mut e = EstiloGrafico::default();
        let (svg, mapa) = g(Tipo::Colunas).exportar_svg_formatado_com_mapa(600.0, 300.0, &e).unwrap();
        assert_eq!(mapa.legenda.len(), 2);
        let alvo = mapa.legenda[1];
        let (cx, cy) = (alvo.x + alvo.largura / 2.0, alvo.y + alvo.altura / 2.0);
        assert_eq!(mapa.item_legenda_em(cx, cy), Some(1));
        assert!(svg.contains("barra s1"));
        e.legenda.alternar(1);
        let (svg, mapa) = g(Tipo::Colunas).exportar_svg_formatado_com_mapa(600.0, 300.0, &e).unwrap();
        assert!(!svg.contains("barra s1") && svg.contains("barra s0"));
        assert!(svg.contains("hachura-legenda") && svg.contains("legenda-amostra desligado"));
        assert_eq!(mapa.legenda.len(), 2, "o item desligado continua na legenda");
        assert!(mapa.pontos.iter().all(|p| p.serie == 0));
        e.legenda.alternar(1);
        assert!(g(Tipo::Colunas).exportar_svg_formatado(600.0, 300.0, &e).unwrap().contains("barra s1"));
    }

    #[test]
    fn mapa_acha_a_barra_o_ponto_da_linha_e_a_fatia_sob_o_mouse() {
        let e = EstiloGrafico::default();
        for tipo in [Tipo::Colunas, Tipo::Barras, Tipo::Linhas, Tipo::Pizza] {
            let mut graf = g(tipo);
            if tipo == Tipo::Pizza {
                graf.series.truncate(1);
            }
            let (_, mapa) = graf.exportar_svg_formatado_com_mapa(600.0, 300.0, &e).unwrap();
            assert!(!mapa.pontos.is_empty(), "{tipo:?}");
            for alvo in &mapa.pontos {
                let (x, y) = match alvo.forma {
                    FormaAlvo::Retangulo { x, y, largura, altura } => (x + largura / 2.0, y + altura / 2.0),
                    FormaAlvo::Circulo { x, y, .. } => (x, y),
                    FormaAlvo::Fatia { x, y, raio_interno, raio, inicio, fim } => {
                        let m = (inicio + fim) / 2.0;
                        let rr = (raio_interno + raio) * 0.5;
                        (x + rr * m.cos(), y + rr * m.sin())
                    }
                };
                let achado = mapa.ponto_em(x, y).unwrap();
                assert_eq!((achado.serie, achado.ponto), (alvo.serie, alvo.ponto), "{tipo:?}");
            }
            assert!(mapa.ponto_em(-50.0, -50.0).is_none());
        }
    }

    #[test]
    fn legenda_por_item_titulo_alinhado_e_texto_alternativo() {
        let mut e = EstiloGrafico::default();
        e.legenda.itens.insert(0, ItemLegenda { oculto: true, ..Default::default() });
        e.legenda.itens.insert(1, ItemLegenda { texto: Some("Filial".into()), cor: Some(CorEstilo::hex(0x00aa00)), oculto: false, desligado: false });
        e.legenda.posicao = Some(PosicaoLegenda::Direita);
        e.titulo.alinhamento = Some(Alinhamento::Esquerda);
        e.subtitulo.texto = Some("2026".into());
        e.texto_alternativo = "Vendas por mês".into();
        let svg = g(Tipo::Colunas).exportar_svg_formatado(600.0, 300.0, &e).unwrap();
        assert!(!svg.contains(">Loja A</text>") && svg.contains(">Filial</text>"));
        assert!(svg.lines().any(|l| l.contains("legenda-amostra") && l.contains("#00aa00")));
        let titulo = svg.lines().find(|l| l.contains("class=\"titulo\"")).unwrap();
        assert!(titulo.contains("text-anchor=\"start\""));
        assert!(svg.contains(">2026</text>"));
        assert!(svg.contains("aria-label=\"Vendas por mês\"") && svg.contains("<desc>Vendas por mês</desc>"));
        e.legenda.posicao = Some(PosicaoLegenda::Nenhuma);
        let svg = g(Tipo::Colunas).exportar_svg_formatado(600.0, 300.0, &e).unwrap();
        assert!(!svg.contains("class=\"legenda\""));
    }

    #[test]
    fn pizza_ponto_destacado_e_percentual() {
        let mut p = g(Tipo::Pizza);
        p.series.truncate(1);
        let mut e = EstiloGrafico::default();
        e.serie_mut(0).pontos.insert(2, EstiloPonto { cor: Some(CorEstilo::hex(0x111111)), destacar: Some(12.0), ..Default::default() });
        e.rotulos.percentual = Some(true);
        e.rotulos.categoria = Some(false);
        let svg = p.exportar_svg_formatado(400.0, 300.0, &e).unwrap();
        assert!(svg.lines().filter(|l| l.contains("class=\"fatia\"")).nth(2).unwrap().contains("#111111"));
        assert!(svg.contains(">50,0%</text>"));
    }

    #[test]
    fn rosca_com_furo_e_um_anel_por_serie() {
        let e = EstiloGrafico { rosca: Some(0.5), ..Default::default() };
        // Uma série: anel só, rótulos da categoria; o furo não é clicável.
        let mut uma = g(Tipo::Pizza);
        uma.series.truncate(1);
        let (svg, mapa) = uma.exportar_svg_formatado_com_mapa(400.0, 300.0, &e).unwrap();
        assert_eq!(svg.matches("class=\"fatia\"").count(), 3);
        assert!(svg.contains(">Jan</text>"));
        let FormaAlvo::Fatia { x, y, raio_interno, raio, .. } = mapa.pontos[0].forma else { panic!("fatia") };
        assert!((raio_interno / raio - 0.5).abs() < 1e-3);
        assert!(mapa.ponto_em(x, y).is_none(), "o centro é o furo");
        // Duas séries: dois anéis (a 1ª por dentro), sem repetir a categoria.
        let duas = g(Tipo::Pizza);
        assert!(duas.exportar_svg_formatado(400.0, 300.0, &EstiloGrafico::default()).is_err(), "pizza continua com uma série");
        let (svg, mapa) = duas.exportar_svg_formatado_com_mapa(400.0, 300.0, &e).unwrap();
        assert_eq!(svg.matches("class=\"fatia\"").count(), 6);
        assert!(!svg.contains("class=\"rotulo\""), "sem repetir a categoria em cada anel");
        let raios = |s: usize| mapa.pontos.iter().find_map(|a| match a.forma {
            FormaAlvo::Fatia { raio_interno, raio, .. } if a.serie == s => Some((raio_interno, raio)),
            _ => None,
        }).unwrap();
        let ((i0, f0), (i1, f1)) = (raios(0), raios(1));
        assert!(i0 < f0 && (f0 - i1).abs() < 1e-3 && i1 < f1, "anéis encostados, a série 0 por dentro");
        for alvo in &mapa.pontos {
            let FormaAlvo::Fatia { x, y, raio_interno, raio, inicio, fim } = alvo.forma else { panic!() };
            let (m, rr) = ((inicio + fim) / 2.0, (raio_interno + raio) / 2.0);
            let achado = mapa.ponto_em(x + rr * m.cos(), y + rr * m.sin()).unwrap();
            assert_eq!((achado.serie, achado.ponto), (alvo.serie, alvo.ponto));
        }
        // A cor da fatia (pintada na 1ª série) vale para os dois anéis.
        let mut pintada = e.clone();
        pintada.serie_mut(0).pontos.insert(1, EstiloPonto { cor: Some(CorEstilo::hex(0x111111)), ..Default::default() });
        let svg = duas.exportar_svg_formatado(400.0, 300.0, &pintada).unwrap();
        assert_eq!(svg.lines().filter(|l| l.contains("class=\"fatia\"") && l.contains("#111111")).count(), 2);
        // Um valor só: anel inteiro com o furo vazado.
        let mut cheia = g(Tipo::Pizza);
        cheia.series.truncate(1);
        cheia.series[0].valores = vec![0.0, 7.0, 0.0];
        let svg = cheia.exportar_svg_formatado(400.0, 300.0, &e).unwrap();
        assert!(svg.contains("fill-rule=\"evenodd\""));
        assert!(!EstiloGrafico { rosca: Some(1.5), ..Default::default() }.valido());
    }

    #[test]
    fn tendencia_com_equacao_barras_de_erro_e_linhas_de_referencia() {
        use crate::analitico::{Analise, BarrasErro, Tendencia};
        let mut e = EstiloGrafico::default();
        e.serie_mut(0).analise = Analise {
            tendencia: Some(Tendencia { mostrar_equacao: true, mostrar_r2: true, ..Default::default() }),
            erro: Some(BarrasErro::default()),
            maximo: true, minimo: true, media: true,
        };
        for tipo in [Tipo::Colunas, Tipo::Barras, Tipo::Linhas, Tipo::Area, Tipo::Dispersao] {
            let mut gr = g(tipo);
            if tipo == Tipo::Dispersao {
                gr.categorias = vec!["1".into(), "2".into(), "3".into()];
            }
            let svg = gr.exportar_svg_formatado(640.0, 360.0, &e).unwrap();
            assert!(svg.contains("class=\"tendencia s0\""), "{tipo:?}");
            assert!(svg.contains("y = 10x + 0") && svg.contains("R² = 1,0000"), "{tipo:?}");
            assert_eq!(svg.matches("class=\"analise-ref\"").count(), 3, "{tipo:?}");
            assert!(svg.contains(">Máx: 30</text>") && svg.contains(">Média: 20</text>"), "{tipo:?}");
            // 3 pontos × (traço + 2 pontas).
            assert_eq!(svg.matches("class=\"analise-erro\"").count(), 9, "{tipo:?}");
            assert!(!svg.contains("NaN"));
        }
        // A série B não tem análise; pizza ignora.
        let svg = g(Tipo::Linhas).exportar_svg_formatado(640.0, 360.0, &e).unwrap();
        assert!(!svg.contains("tendencia s1"));
        let mut pizza = g(Tipo::Pizza);
        pizza.series.truncate(1);
        assert!(!pizza.exportar_svg_formatado(640.0, 360.0, &e).unwrap().contains("tendencia"));
        // Sem análise o desenho é o de sempre.
        let limpo = g(Tipo::Linhas).exportar_svg_formatado(640.0, 360.0, &EstiloGrafico::default()).unwrap();
        assert!(!limpo.contains("analise") && !limpo.contains("tendencia"));
    }

    #[test]
    fn rejeita_estilo_invalido() {
        let mut e = EstiloGrafico::default();
        e.eixo_y.minimo = Some(5.0);
        e.eixo_y.maximo = Some(1.0);
        assert_eq!(g(Tipo::Colunas).exportar_svg_formatado(600.0, 300.0, &e), Err(ErroGrafico::ValorInvalido));
    }
}
