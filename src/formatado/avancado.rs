//! Tipos avançados no desenho formatado: combinado, histograma, cascata,
//! radar, bolhas, velas, Gantt, mapa de calor e linha 3D.
//!
//! A moldura (fundo, título, legenda, tema, fonte, margens) é a mesma dos
//! tipos básicos; aqui fica só o miolo de cada tipo, com eixos, grades e
//! rótulos no mesmo estilo e o mapa das partes sob o mouse.
use super::*;

/// Tipos que não cabem no desenho cartesiano comum.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TipoAvancado {
    /// Colunas na primeira série e linhas nas demais.
    Combinado,
    /// Frequência dos valores da primeira série, em faixas.
    Histograma,
    /// Cada valor da primeira série soma ao saldo anterior; termina no total.
    Cascata,
    Radar,
    /// X real, Y na primeira série e tamanho no Z (ou na segunda série).
    Bolhas,
    /// Abertura, máxima, mínima e fechamento nas quatro primeiras séries.
    Velas,
    /// Início e término nas duas primeiras séries, uma tarefa por categoria.
    Gantt,
    /// Categorias nas colunas e séries nas linhas, cor pela intensidade.
    MapaCalor,
    /// X, Y e Z em projeção isométrica.
    Linha3d,
}

impl TipoAvancado {
    pub(super) fn tem_legenda(self) -> bool {
        !matches!(self, Self::Cascata | Self::Velas | Self::Gantt | Self::MapaCalor | Self::Histograma)
    }
    /// Só estas séries entram na legenda (bolhas: o tamanho não é série).
    pub(super) fn itens_legenda(self, _total: usize) -> Option<Vec<usize>> {
        match self {
            Self::Bolhas => Some(vec![0]),
            _ => None,
        }
    }
}

/// O pedido de desenho de um tipo avançado e os dados que não são séries.
#[derive(Debug, Clone, PartialEq)]
pub struct PedidoAvancado {
    pub tipo: TipoAvancado,
    /// X numérico (bolhas, linha 3D); sem ele, as categorias como número.
    pub x: Option<Vec<f64>>,
    /// Z (tamanho das bolhas, profundidade da linha 3D).
    pub z: Option<Vec<f64>>,
    /// Gantt: o dia de hoje (no mesmo número das datas, o serial do Excel) e o
    /// rótulo da linha, na língua do aplicativo. Só vale com [`crate::EstiloGantt`] ligado.
    pub hoje: Option<(f64, String)>,
}

impl PedidoAvancado {
    pub fn novo(tipo: TipoAvancado) -> Self {
        Self { tipo, x: None, z: None, hoje: None }
    }
    pub(super) fn validar(&self, g: &Grafico) -> Result<(), ErroGrafico> {
        let n = g.categorias.len();
        let tam = |v: &Option<Vec<f64>>| v.as_ref().map_or(true, |v| v.len() == n && v.iter().all(|x| x.is_finite()));
        if !tam(&self.x) || !tam(&self.z) {
            return Err(ErroGrafico::TamanhosDiferentes);
        }
        match self.tipo {
            TipoAvancado::Velas if g.series.len() < 4 => Err(ErroGrafico::SemDados),
            TipoAvancado::Gantt if g.series.len() < 2 => Err(ErroGrafico::SemDados),
            TipoAvancado::Gantt if g.series[0].valores.iter().zip(&g.series[1].valores).any(|(a, b)| b < a) => Err(ErroGrafico::ValorInvalido),
            TipoAvancado::Linha3d if self.z.is_none() => Err(ErroGrafico::SemDados),
            _ => Ok(()),
        }
    }
    fn xs(&self, g: &Grafico) -> Vec<f64> {
        self.x.clone().unwrap_or_else(|| {
            g.categorias.iter().enumerate().map(|(i, c)| c.trim().replace(',', ".").parse().unwrap_or(i as f64 + 1.0)).collect()
        })
    }
}

/// O que a dica mostra quando o ponto sob o mouse não é um valor das séries
/// de entrada: `series[s].1[p]` é o valor do ponto `p` da série `s`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct DadosDica {
    pub categorias: Vec<String>,
    pub series: Vec<(String, Vec<f64>)>,
    /// `datas[s]`: os valores da série `s` são datas (número de série do
    /// Excel), mostradas como dd/mm/aaaa. Ausente = número.
    pub datas: Vec<bool>,
}

/// Número inteiro entre 1950 e 2150 como data do Excel (dias desde
/// 30/12/1899): é o que uma célula de data entrega ao gráfico.
fn parece_data(v: f64) -> bool {
    v.fract() == 0.0 && (18_264.0..=91_311.0).contains(&v)
}

pub(super) fn desenhar(g: &Grafico, d: &mut Desenho, r: &Resolvido<'_>, caixa: Ret, p: &PedidoAvancado) {
    match p.tipo {
        TipoAvancado::Combinado => combinado(g, d, r, caixa),
        TipoAvancado::Histograma => histograma(g, d, r, caixa),
        TipoAvancado::Cascata => cascata(g, d, r, caixa),
        TipoAvancado::Radar => radar(g, d, r, caixa),
        TipoAvancado::Bolhas => bolhas(g, d, r, caixa, p),
        TipoAvancado::Velas => velas(g, d, r, caixa),
        TipoAvancado::Gantt => gantt(g, d, r, caixa, p.hoje.as_ref()),
        TipoAvancado::MapaCalor => mapa_calor(g, d, r, caixa),
        TipoAvancado::Linha3d => linha_3d(g, d, r, caixa, p),
    }
}

// --------------------------------------------------------------- eixos

/// Área de plotagem com o eixo de valores à esquerda (grade, rótulos e
/// título) e espaço embaixo para os rótulos do X.
struct Plano {
    plot: Ret,
    dados: Ret,
    ey: Escala,
}
impl Plano {
    fn y(&self, v: f64) -> f32 {
        self.dados.y + self.dados.h * (1.0 - self.ey.fracao(v))
    }
}

fn fonte_eixo(r: &Resolvido<'_>, x: bool) -> Fonte {
    let e = r.e;
    Fonte::de(&r.base, if x { &e.eixo_x.rotulos } else { &e.eixo_y.rotulos }, 1.0)
}

fn plano(d: &mut Desenho, r: &Resolvido<'_>, caixa: Ret, min: f64, max: f64, incluir_zero: bool, rotulos_x: bool) -> Plano {
    let e = r.e;
    let ey = Escala::nova(min, max, &e.eixo_y, incluir_zero);
    let fy = fonte_eixo(r, false);
    let fx = fonte_eixo(r, true);
    let vis_y = e.eixo_y.visivel.unwrap_or(true);
    let fmt = e.eixo_y.formato.unwrap_or_default();
    let rot: Vec<String> = ey.ticks.iter().map(|v| formatar(*v, fmt, ey.casas)).collect();
    let mut esq = if vis_y { rot.iter().map(|t| largura_texto(t, fy.tamanho)).fold(0.0, f32::max) + 8.0 } else { 0.0 };
    let tit_y = e.eixo_y.titulo.clone().filter(|t| !t.trim().is_empty());
    let tit_x = e.eixo_x.titulo.clone().filter(|t| !t.trim().is_empty());
    let f_ty = Fonte::de(&r.base, &e.eixo_y.fonte_titulo, 1.0);
    let f_tx = Fonte::de(&r.base, &e.eixo_x.fonte_titulo, 1.0);
    if tit_y.is_some() {
        esq += f_ty.tamanho * 1.4;
    }
    let mut inf = if rotulos_x && e.eixo_x.visivel.unwrap_or(true) { fx.tamanho * 1.6 } else { 4.0 };
    if tit_x.is_some() {
        inf += f_tx.tamanho * 1.4;
    }
    let plot = caixa.encolher(fy.tamanho * 0.6, inf, esq, 8.0);
    let mp = &e.margens_plotagem;
    let dados = plot.encolher(mp.superior.unwrap_or(0.0), mp.inferior.unwrap_or(0.0), mp.esquerda.unwrap_or(0.0), mp.direita.unwrap_or(0.0));
    let b = r.linha(&e.area_plotagem.borda, false, r.eixo_cor(), 1.0);
    d.area(plot, "plotagem", e.area_plotagem.preenchimento.as_ref(), r.tema.and_then(|t| t.plotagem).map(|c| c.0),
        b, e.area_plotagem.sombra.as_ref(), e.area_plotagem.raio.unwrap_or(0.0));
    let pl = Plano { plot, dados, ey };
    // Grade: a do tema, ou a ligada no painel.
    let cor_grade = r.tema.map_or(GRADE_PADRAO, |t| t.grade.0);
    let grade_vis = r.tema.map_or(true, |t| t.grade_visivel);
    if let Some((c, es, t)) = r.linha(&e.eixo_y.grade_principal, grade_vis, cor_grade, 1.0) {
        for v in &pl.ey.ticks {
            let y = pl.y(*v);
            d.linha(plot.x, y, plot.x + plot.w, y, c, es, t, "grade-y");
        }
    }
    if vis_y {
        for (v, t) in pl.ey.ticks.iter().zip(&rot) {
            d.texto(plot.x - 6.0, pl.y(*v) + fy.tamanho * 0.35, t, &fy, Ancora::Fim, 0.0, "rotulo-y");
        }
    }
    if let Some((c, es, t)) = r.linha(&e.eixo_x.linha, true, r.eixo_cor(), 1.0) {
        let y = pl.y(pl.ey.base(e.eixo_y.cruzamento));
        d.linha(plot.x, y, plot.x + plot.w, y, c, es, t, "eixo-x");
    }
    if let Some(t) = &tit_y {
        d.texto(caixa.x + f_ty.tamanho * 0.95, plot.y + plot.h / 2.0, t, &f_ty, Ancora::Meio, -90.0, "titulo-eixo");
    }
    if let Some(t) = &tit_x {
        d.texto(plot.x + plot.w / 2.0, caixa.y + caixa.h - f_tx.tamanho * 0.25, t, &f_tx, Ancora::Meio, 0.0, "titulo-eixo");
    }
    pl
}

/// Rótulos embaixo da plotagem, ralos quando não cabem.
fn rotulos_x(d: &mut Desenho, r: &Resolvido<'_>, pl: &Plano, itens: &[(f32, String)]) {
    if !r.e.eixo_x.visivel.unwrap_or(true) || itens.is_empty() {
        return;
    }
    let f = fonte_eixo(r, true);
    let maior = itens.iter().map(|(_, t)| largura_texto(t, f.tamanho)).fold(0.0, f32::max).min(pl.plot.w * 0.4);
    let espaco = pl.dados.w / itens.len() as f32;
    let passo = (((maior + 8.0) / espaco.max(1.0)).ceil() as usize).max(1);
    let lim = espaco * passo as f32 - 4.0;
    for (x, t) in itens.iter().step_by(passo) {
        d.texto(*x, pl.plot.y + pl.plot.h + f.tamanho * 1.15, &cortar(t, lim, f.tamanho), &f, Ancora::Meio, 0.0, "rotulo-x");
    }
}

fn faixa(vals: impl Iterator<Item = f64>) -> (f64, f64) {
    let (a, b) = vals.filter(|v| v.is_finite()).fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), v| (a.min(v), b.max(v)));
    if a.is_finite() { (a, b) } else { (0.0, 1.0) }
}

fn rotulo_valor(d: &mut Desenho, r: &Resolvido<'_>, s: usize, x: f32, y: f32, v: f64) {
    let rot = r.rotulos_da_serie(s);
    if !rot.visivel.unwrap_or(false) {
        return;
    }
    let f = Fonte::de(&r.base, &rot.fonte, 0.92);
    let t = formatar(v, rot.formato.or(r.e.eixo_y.formato).unwrap_or_default(), casas_de(v));
    d.texto(x, y, &t, &f, Ancora::Meio, 0.0, "rotulo");
}

fn abrir_recorte(d: &mut Desenho, plot: Ret) {
    let _ = write!(d.defs, "<clipPath id=\"recorte-plotagem\"><rect x=\"{:.2}\" y=\"{:.2}\" width=\"{:.2}\" height=\"{:.2}\"/></clipPath>", plot.x - 1.0, plot.y - 1.0, plot.w + 2.0, plot.h + 2.0);
    d.corpo.push_str("<g clip-path=\"url(#recorte-plotagem)\">\n");
}

// ----------------------------------------------------------------- tipos

fn combinado(g: &Grafico, d: &mut Desenho, r: &Resolvido<'_>, caixa: Ret) {
    let n = g.categorias.len();
    let ligada = |s: usize| r.ativo(s);
    let (a, b) = faixa(g.series.iter().enumerate().filter(|(i, _)| ligada(*i)).flat_map(|(_, s)| s.valores.iter().copied()));
    let pl = plano(d, r, caixa, a, b, true, true);
    let banda = pl.dados.w / n as f32;
    let cx = |i: usize| pl.dados.x + banda * (i as f32 + 0.5);
    abrir_recorte(d, pl.plot);
    let base = pl.y(pl.ey.base(r.e.eixo_y.cruzamento));
    let raio = r.e.barras.raio.unwrap_or(r.tema.map_or(0.0, |t| t.raio_barras));
    if ligada(0) {
        let bw = (banda * 0.6).min(90.0);
        for (i, v) in g.series[0].valores.iter().enumerate() {
            let y = pl.y(*v);
            let (y0, h) = (y.min(base), (y - base).abs());
            let x = cx(i) - bw / 2.0;
            let _ = writeln!(d.corpo, "<rect class=\"barra s0 p{i}\" x=\"{x:.2}\" y=\"{y0:.2}\" width=\"{bw:.2}\" height=\"{h:.2}\" rx=\"{:.2}\" {}/>",
                raio.min(bw / 2.0).min(h / 2.0), preencher(r.cor_ponto(g, 0, i)));
            d.mapa.pontos.push(AlvoPonto { serie: 0, ponto: i, forma: FormaAlvo::Retangulo { x, y: y0, largura: bw, altura: h } });
        }
    }
    for (s, serie) in g.series.iter().enumerate().skip(1).filter(|(s, _)| ligada(*s)) {
        let cor = r.cor_serie(g, s);
        let est = r.e.serie(s);
        let pts: Vec<(f32, f32)> = serie.valores.iter().enumerate().map(|(i, v)| (cx(i), pl.y(*v))).collect();
        let esp = est.and_then(|x| x.espessura_linha).unwrap_or(2.4);
        let _ = writeln!(d.corpo, "<path class=\"linha s{s}\" d=\"{}\" fill=\"none\" {} stroke-linejoin=\"round\" stroke-linecap=\"round\"/>",
            caminho(&pts, est.and_then(|x| x.suavizar).unwrap_or(false)), tracar(cor, esp, est.and_then(|x| x.traco).unwrap_or_default()));
        let forma = est.and_then(|x| x.marcador).unwrap_or(FormaMarcador::Circulo);
        for (i, p) in pts.iter().enumerate() {
            marcador(d, forma, p.0, p.1, est.and_then(|x| x.tamanho_marcador).unwrap_or(6.0), r.cor_ponto(g, s, i), None);
            d.mapa.pontos.push(AlvoPonto { serie: s, ponto: i, forma: FormaAlvo::Circulo { x: p.0, y: p.1, raio: 7.0 } });
        }
    }
    d.corpo.push_str("</g>\n");
    for (s, serie) in g.series.iter().enumerate().filter(|(s, _)| ligada(*s)) {
        for (i, v) in serie.valores.iter().enumerate() {
            rotulo_valor(d, r, s, cx(i), pl.y(*v) - 5.0, *v);
        }
    }
    let itens: Vec<(f32, String)> = (0..n).map(|i| (cx(i), g.categorias[i].clone())).collect();
    rotulos_x(d, r, &pl, &itens);
}

fn histograma(g: &Grafico, d: &mut Desenho, r: &Resolvido<'_>, caixa: Ret) {
    let vals: Vec<f64> = g.series[0].valores.clone();
    let (min, max) = faixa(vals.iter().copied());
    let faixas = ((vals.len() as f64).sqrt().ceil() as usize).clamp(1, 32);
    let largura = if max > min { (max - min) / faixas as f64 } else { 1.0 };
    let mut freq = vec![0.0f64; faixas];
    for v in &vals {
        let k = if max > min { ((v - min) / largura) as usize } else { 0 };
        freq[k.min(faixas - 1)] += 1.0;
    }
    let fmt = r.e.eixo_x.formato.unwrap_or_default();
    let nomes: Vec<String> = (0..faixas).map(|k| {
        let a = min + largura * k as f64;
        format!("{}–{}", formatar(a, fmt, casas_de(largura)), formatar(a + largura, fmt, casas_de(largura)))
    }).collect();
    let pl = plano(d, r, caixa, 0.0, freq.iter().copied().fold(1.0, f64::max), true, true);
    let banda = pl.dados.w / faixas as f32;
    let cor = r.cor_serie(g, 0);
    let base = pl.y(0.0);
    abrir_recorte(d, pl.plot);
    for (k, f) in freq.iter().enumerate() {
        let x = pl.dados.x + banda * k as f32 + 0.5;
        let y = pl.y(*f);
        let (bw, h) = ((banda - 1.0).max(1.0), base - y);
        let _ = writeln!(d.corpo, "<rect class=\"barra s0 p{k}\" x=\"{x:.2}\" y=\"{y:.2}\" width=\"{bw:.2}\" height=\"{h:.2}\" {}/>", preencher(r.cor_ponto(g, 0, k)));
        let _ = cor;
        d.mapa.pontos.push(AlvoPonto { serie: 0, ponto: k, forma: FormaAlvo::Retangulo { x, y, largura: bw, altura: h } });
    }
    d.corpo.push_str("</g>\n");
    for (k, f) in freq.iter().enumerate() {
        rotulo_valor(d, r, 0, pl.dados.x + banda * (k as f32 + 0.5), pl.y(*f) - 5.0, *f);
    }
    let itens: Vec<(f32, String)> = nomes.iter().enumerate().map(|(k, t)| (pl.dados.x + banda * (k as f32 + 0.5), t.clone())).collect();
    rotulos_x(d, r, &pl, &itens);
    d.mapa.dica = Some(DadosDica { categorias: nomes, series: vec![("Frequência".into(), freq)], ..Default::default() });
}

fn cascata(g: &Grafico, d: &mut Desenho, r: &Resolvido<'_>, caixa: Ret) {
    let vals = &g.series[0].valores;
    let n = vals.len();
    let mut saldos = Vec::with_capacity(n + 1);
    let mut saldo = 0.0;
    for v in vals {
        let antes = saldo;
        saldo += v;
        saldos.push((antes, saldo));
    }
    saldos.push((0.0, saldo));
    let (a, b) = faixa(saldos.iter().flat_map(|(x, y)| [*x, *y]));
    let pl = plano(d, r, caixa, a, b, true, true);
    let total = n + 1;
    let banda = pl.dados.w / total as f32;
    let bw = (banda * 0.6).min(90.0);
    let paleta = r.tema.map(|t| t.paleta);
    let sobe = r.e.serie(0).and_then(|s| s.cor).map(|c| c.0).unwrap_or(Cor::rgba(24, 150, 90, 255));
    let desce = paleta.map_or(Cor::rgba(220, 70, 72, 255), |_| Cor::rgba(214, 69, 65, 255));
    let cor_total = r.eixo_cor();
    abrir_recorte(d, pl.plot);
    let mut anterior: Option<f32> = None;
    for (i, (de, ate)) in saldos.iter().enumerate() {
        let x = pl.dados.x + banda * i as f32 + (banda - bw) / 2.0;
        let (y0, y1) = (pl.y(*de), pl.y(*ate));
        let (y, h) = (y0.min(y1), (y0 - y1).abs().max(1.0));
        let cor = if i == n { cor_total } else if ate >= de { sobe } else { desce };
        let cor = r.e.serie(0).and_then(|s| s.pontos.get(&i)).and_then(|p| p.cor).map_or(cor, |c| c.0);
        let _ = writeln!(d.corpo, "<rect class=\"barra s0 p{i}\" x=\"{x:.2}\" y=\"{y:.2}\" width=\"{bw:.2}\" height=\"{h:.2}\" rx=\"2\" {}/>", preencher(cor));
        if let Some(ya) = anterior {
            // Conector do saldo anterior até a barra.
            d.linha(x - (banda - bw), ya, x, ya, com_alfa(cor_total, 0.7), 1.0, Traco::Tracejado, "conector");
        }
        anterior = Some(y1);
        d.mapa.pontos.push(AlvoPonto { serie: 0, ponto: i, forma: FormaAlvo::Retangulo { x, y, largura: bw, altura: h } });
    }
    d.corpo.push_str("</g>\n");
    for (i, (de, ate)) in saldos.iter().enumerate() {
        let v = if i == n { *ate } else { ate - de };
        rotulo_valor(d, r, 0, pl.dados.x + banda * (i as f32 + 0.5), pl.y(de.max(*ate)) - 5.0, v);
    }
    let mut nomes = g.categorias.clone();
    nomes.push("Total".into());
    let itens: Vec<(f32, String)> = nomes.iter().enumerate().map(|(i, t)| (pl.dados.x + banda * (i as f32 + 0.5), t.clone())).collect();
    rotulos_x(d, r, &pl, &itens);
    let mut valores: Vec<f64> = vals.clone();
    valores.push(saldo);
    let acumulado: Vec<f64> = saldos.iter().map(|(_, ate)| *ate).collect();
    d.mapa.dica = Some(DadosDica {
        categorias: nomes,
        series: vec![(g.series[0].nome.clone(), valores), ("Acumulado".into(), acumulado)],
        ..Default::default()
    });
}

fn radar(g: &Grafico, d: &mut Desenho, r: &Resolvido<'_>, caixa: Ret) {
    let n = g.categorias.len();
    let f = fonte_eixo(r, true);
    let mp = &r.e.margens_plotagem;
    let area = caixa.encolher(mp.superior.unwrap_or(0.0), mp.inferior.unwrap_or(0.0), mp.esquerda.unwrap_or(0.0), mp.direita.unwrap_or(0.0));
    let rotulo_w = g.categorias.iter().map(|c| largura_texto(c, f.tamanho)).fold(0.0, f32::max).min(area.w * 0.25);
    let (cx, cy) = (area.x + area.w / 2.0, area.y + area.h / 2.0);
    let raio = ((area.w / 2.0 - rotulo_w - 6.0).min(area.h / 2.0 - f.tamanho * 1.6)).max(10.0);
    let (a, b) = faixa(g.series.iter().enumerate().filter(|(i, _)| r.ativo(*i)).flat_map(|(_, s)| s.valores.iter().copied()));
    let esc = Escala::nova(a.min(0.0), b, &r.e.eixo_y, true);
    let ang = |i: usize| -std::f32::consts::FRAC_PI_2 + std::f32::consts::TAU * i as f32 / n.max(1) as f32;
    let pt = |i: usize, v: f64| {
        let k = raio * esc.fracao(v);
        (cx + k * ang(i).cos(), cy + k * ang(i).sin())
    };
    let cor_grade = r.tema.map_or(GRADE_PADRAO, |t| t.grade.0);
    let fy = fonte_eixo(r, false);
    let fmt = r.e.eixo_y.formato.unwrap_or_default();
    for v in &esc.ticks {
        let anel: Vec<(f32, f32)> = (0..n).map(|i| pt(i, *v)).collect();
        let mut dd = caminho(&anel, false);
        dd.push_str(" Z");
        let _ = writeln!(d.corpo, "<path class=\"grade-radar\" d=\"{dd}\" fill=\"none\" {}/>", tracar(cor_grade, 1.0, Traco::Continuo));
        // O rótulo do anel de fora cairia em cima da primeira categoria.
        if r.e.eixo_y.visivel.unwrap_or(true) && *v < esc.max {
            let (x, y) = pt(0, *v);
            d.texto(x + 4.0, y + fy.tamanho * 0.35, &formatar(*v, fmt, esc.casas), &fy, Ancora::Inicio, 0.0, "rotulo-y");
        }
    }
    for i in 0..n {
        let (x, y) = pt(i, esc.max);
        d.linha(cx, cy, x, y, cor_grade, 1.0, Traco::Continuo, "raio");
        let (lx, ly) = (cx + (raio + 8.0) * ang(i).cos(), cy + (raio + 8.0) * ang(i).sin());
        let anc = if ang(i).cos() > 0.2 { Ancora::Inicio } else if ang(i).cos() < -0.2 { Ancora::Fim } else { Ancora::Meio };
        d.texto(lx, ly + f.tamanho * 0.35, &cortar(&g.categorias[i], rotulo_w + 4.0, f.tamanho), &f, anc, 0.0, "rotulo-x");
    }
    for (s, serie) in g.series.iter().enumerate().filter(|(s, _)| r.ativo(*s)) {
        let cor = r.cor_serie(g, s);
        let pts: Vec<(f32, f32)> = serie.valores.iter().enumerate().map(|(i, v)| pt(i, *v)).collect();
        let mut dd = caminho(&pts, false);
        dd.push_str(" Z");
        let esp = r.e.serie(s).and_then(|x| x.espessura_linha).unwrap_or(2.0);
        let _ = writeln!(d.corpo, "<path class=\"linha s{s}\" d=\"{dd}\" {} {} stroke-linejoin=\"round\"/>", preencher(com_alfa(cor, 0.18)), tracar(cor, esp, Traco::Continuo));
        for (i, p) in pts.iter().enumerate() {
            marcador(d, FormaMarcador::Circulo, p.0, p.1, 5.0, cor, None);
            d.mapa.pontos.push(AlvoPonto { serie: s, ponto: i, forma: FormaAlvo::Circulo { x: p.0, y: p.1, raio: 7.0 } });
        }
    }
}

fn bolhas(g: &Grafico, d: &mut Desenho, r: &Resolvido<'_>, caixa: Ret, p: &PedidoAvancado) {
    let xs = p.xs(g);
    let ys = &g.series[0].valores;
    let tam: Vec<f64> = p.z.clone().or_else(|| g.series.get(1).map(|s| s.valores.clone())).unwrap_or_else(|| vec![1.0; ys.len()]);
    let maior = tam.iter().map(|v| v.abs()).fold(0.0, f64::max).max(f64::MIN_POSITIVE);
    // Folga nos dois eixos: a bolha da ponta não sai cortada.
    let folga = |(a, b): (f64, f64)| { let m = ((b - a) * 0.12).max(f64::EPSILON); (a - m, b + m) };
    let (a, b) = folga(faixa(ys.iter().copied()));
    let pl = plano(d, r, caixa, a, b, false, true);
    let (xa, xb) = folga(faixa(xs.iter().copied()));
    let ex = Escala::nova(xa, xb, &r.e.eixo_x, false);
    let xp = |v: f64| pl.dados.x + pl.dados.w * ex.fracao(v);
    let raio_max = (pl.dados.w.min(pl.dados.h) * 0.09).clamp(8.0, 40.0);
    let cor = r.cor_serie(g, 0);
    abrir_recorte(d, pl.plot);
    // As maiores primeiro, para as pequenas ficarem por cima.
    let mut ordem: Vec<usize> = (0..ys.len()).collect();
    ordem.sort_by(|i, j| tam[*j].abs().total_cmp(&tam[*i].abs()));
    if r.ativo(0) {
        for i in ordem {
            let (x, y) = (xp(xs[i]), pl.y(ys[i]));
            let rr = 3.0 + (raio_max - 3.0) * (tam[i].abs() / maior).sqrt() as f32;
            let c = r.cor_ponto(g, 0, i);
            let _ = writeln!(d.corpo, "<circle class=\"bolha s0 p{i}\" cx=\"{x:.2}\" cy=\"{y:.2}\" r=\"{rr:.2}\" {} {}/>", preencher(com_alfa(c, 0.6)), tracar(c, 1.2, Traco::Continuo));
            d.mapa.pontos.push(AlvoPonto { serie: 0, ponto: i, forma: FormaAlvo::Circulo { x, y, raio: rr.max(5.0) } });
        }
    }
    d.corpo.push_str("</g>\n");
    let _ = cor;
    let fmt = r.e.eixo_x.formato.unwrap_or_default();
    let itens: Vec<(f32, String)> = ex.ticks.iter().map(|v| (xp(*v), formatar(*v, fmt, ex.casas))).collect();
    rotulos_x(d, r, &pl, &itens);
    let nome_t = g.series.get(1).map_or_else(|| "Tamanho".to_owned(), |s| s.nome.clone());
    d.mapa.dica = Some(DadosDica {
        categorias: g.categorias.iter().zip(&xs).map(|(c, x)| if c.trim().is_empty() { formatar(*x, fmt, casas_de(*x)) } else { c.clone() }).collect(),
        series: vec![(g.series[0].nome.clone(), ys.clone()), (nome_t, tam)],
        ..Default::default()
    });
}

fn velas(g: &Grafico, d: &mut Desenho, r: &Resolvido<'_>, caixa: Ret) {
    let (ab, hi, lo, fe) = (&g.series[0].valores, &g.series[1].valores, &g.series[2].valores, &g.series[3].valores);
    let (a, _) = faixa(lo.iter().copied());
    let (_, b) = faixa(hi.iter().copied());
    let pl = plano(d, r, caixa, a, b, false, true);
    let n = g.categorias.len();
    let banda = pl.dados.w / n as f32;
    let bw = (banda * 0.55).clamp(2.0, 40.0);
    let sobe = r.e.serie(3).and_then(|s| s.cor).map_or(Cor::rgba(32, 160, 106, 255), |c| c.0);
    let desce = r.e.serie(0).and_then(|s| s.cor).map_or(Cor::rgba(215, 65, 70, 255), |c| c.0);
    abrir_recorte(d, pl.plot);
    for i in 0..n {
        let x = pl.dados.x + banda * (i as f32 + 0.5);
        let cor = if fe[i] >= ab[i] { sobe } else { desce };
        d.linha(x, pl.y(hi[i]), x, pl.y(lo[i]), cor, 1.4, Traco::Continuo, "pavio");
        let (y0, y1) = (pl.y(ab[i]), pl.y(fe[i]));
        let (y, h) = (y0.min(y1), (y0 - y1).abs().max(1.0));
        let _ = writeln!(d.corpo, "<rect class=\"vela p{i}\" x=\"{:.2}\" y=\"{y:.2}\" width=\"{bw:.2}\" height=\"{h:.2}\" {}/>", x - bw / 2.0, preencher(cor));
        let topo = pl.y(hi[i]);
        d.mapa.pontos.push(AlvoPonto { serie: 3, ponto: i, forma: FormaAlvo::Retangulo { x: x - bw / 2.0, y: topo, largura: bw, altura: (pl.y(lo[i]) - topo).max(4.0) } });
    }
    d.corpo.push_str("</g>\n");
    let itens: Vec<(f32, String)> = (0..n).map(|i| (pl.dados.x + banda * (i as f32 + 0.5), g.categorias[i].clone())).collect();
    rotulos_x(d, r, &pl, &itens);
    d.mapa.dica = Some(DadosDica {
        categorias: g.categorias.clone(),
        series: ["Abertura", "Máxima", "Mínima", "Fechamento"].iter().zip([ab, hi, lo, fe])
            .map(|(nome, v)| ((*nome).to_owned(), v.clone())).collect(),
        ..Default::default()
    });
}

fn gantt(g: &Grafico, d: &mut Desenho, r: &Resolvido<'_>, caixa: Ret, hoje: Option<&(f64, String)>) {
    let e = r.e;
    let (ini, fim) = (&g.series[0].valores, &g.series[1].valores);
    let n = g.categorias.len();
    let (a, _) = faixa(ini.iter().copied());
    let (_, b) = faixa(fim.iter().copied());
    let ex = Escala::nova(a, b, &e.eixo_y, false);
    let f = fonte_eixo(r, true);
    let fv = fonte_eixo(r, false);
    let esq = g.categorias.iter().map(|c| largura_texto(c, f.tamanho)).fold(0.0, f32::max).min(caixa.w * 0.35) + 10.0;
    let plot = caixa.encolher(4.0, fv.tamanho * 1.6, esq, 10.0);
    let b2 = r.linha(&e.area_plotagem.borda, false, r.eixo_cor(), 1.0);
    d.area(plot, "plotagem", e.area_plotagem.preenchimento.as_ref(), r.tema.and_then(|t| t.plotagem).map(|c| c.0), b2, None, 0.0);
    let xp = |v: f64| plot.x + plot.w * ex.fracao(v);
    let cor_grade = r.tema.map_or(GRADE_PADRAO, |t| t.grade.0);
    let fmt = e.eixo_y.formato.unwrap_or_default();
    // Início e término em células de data chegam como número de série: no
    // eixo viram dd/mm/aaaa (antes saía 46.030), salvo formato escolhido.
    let datas = ini.iter().chain(fim).all(|v| parece_data(*v));
    let eixo_de_datas = datas && fmt == FormatoNum::Automatico;
    for v in &ex.ticks {
        d.linha(xp(*v), plot.y, xp(*v), plot.y + plot.h, cor_grade, 1.0, Traco::Continuo, "grade-x");
        let rotulo = if eixo_de_datas { super::data_de_serie(*v) } else { formatar(*v, fmt, ex.casas) };
        d.texto(xp(*v), plot.y + plot.h + fv.tamanho * 1.15, &rotulo, &fv, Ancora::Meio, 0.0, "rotulo-x");
    }
    let banda = plot.h / n as f32;
    let bh = (banda * 0.6).min(28.0);
    let raio = e.barras.raio.unwrap_or(r.tema.map_or(3.0, |t| t.raio_barras.max(3.0)));
    for i in 0..n {
        let y = plot.y + banda * i as f32 + (banda - bh) / 2.0;
        d.texto(plot.x - 6.0, plot.y + banda * (i as f32 + 0.5) + f.tamanho * 0.35, &cortar(&g.categorias[i], esq - 8.0, f.tamanho), &f, Ancora::Fim, 0.0, "rotulo-y");
        let (x0, x1) = (xp(ini[i]), xp(fim[i]));
        let w = (x1 - x0).max(2.0);
        let cor = cor_da_tarefa(g, r, i, ini[i], fim[i], hoje.map(|h| h.0));
        let _ = writeln!(d.corpo, "<rect class=\"tarefa p{i}\" x=\"{x0:.2}\" y=\"{y:.2}\" width=\"{w:.2}\" height=\"{bh:.2}\" rx=\"{:.2}\" {}/>", raio.min(bh / 2.0), preencher(cor));
        d.mapa.pontos.push(AlvoPonto { serie: 0, ponto: i, forma: FormaAlvo::Retangulo { x: x0, y, largura: w, altura: bh } });
    }
    if let Some((dia, rotulo)) = hoje.filter(|_| e.gantt.linha_hoje) {
        if (a..=b).contains(dia) {
            let x = xp(*dia);
            let cor = e.gantt.cor_hoje.map_or(COR_HOJE, |c| c.0);
            d.linha(x, plot.y, x, plot.y + plot.h, cor, 1.5, Traco::Tracejado, "hoje");
            let mut fh = fv.clone();
            fh.cor = cor;
            d.texto(x, plot.y - 3.0, rotulo, &fh, Ancora::Meio, 0.0, "rotulo-hoje");
        }
    }
    // Na dica as datas viram dd/mm/aaaa, e a duração, dias.
    let duracao = if datas { "Duração (dias)" } else { "Duração" };
    d.mapa.dica = Some(DadosDica {
        categorias: g.categorias.clone(),
        series: vec![
            ("Início".into(), ini.clone()),
            ("Término".into(), fim.clone()),
            (duracao.into(), ini.iter().zip(fim).map(|(a, b)| b - a).collect()),
        ],
        datas: vec![datas, datas, false],
    });
}

const COR_HOJE: Cor = Cor::rgba(211, 47, 47, 255);
const COR_CONCLUIDA: Cor = Cor::rgba(67, 160, 71, 255);
const COR_FUTURA: Cor = Cor::rgba(176, 190, 197, 255);

/// A cor da barra: a pintada à mão vence; depois a da situação (se ligada); senão a da série.
fn cor_da_tarefa(g: &Grafico, r: &Resolvido<'_>, i: usize, ini: f64, fim: f64, hoje: Option<f64>) -> Cor {
    let pintada = r.e.serie(0).and_then(|s| s.pontos.get(&i)).is_some_and(|p| p.cor.is_some());
    let gt = &r.e.gantt;
    match hoje.filter(|_| gt.pintar_situacao && !pintada) {
        Some(h) if fim < h => gt.concluida.map_or(COR_CONCLUIDA, |c| c.0),
        Some(h) if ini > h => gt.futura.map_or(COR_FUTURA, |c| c.0),
        Some(_) => gt.andamento.map_or_else(|| r.cor_serie(g, 0), |c| c.0),
        None => r.cor_ponto(g, 0, i),
    }
}

fn mapa_calor(g: &Grafico, d: &mut Desenho, r: &Resolvido<'_>, caixa: Ret) {
    // Como a tabela: categorias nas linhas, séries nas colunas.
    let f = fonte_eixo(r, true);
    let fy = fonte_eixo(r, false);
    let (nl, nc) = (g.categorias.len(), g.series.len());
    let esq = g.categorias.iter().map(|c| largura_texto(c, fy.tamanho)).fold(0.0, f32::max).min(caixa.w * 0.3) + 10.0;
    let plot = caixa.encolher(4.0, f.tamanho * 1.6, esq, 4.0);
    let (a, b) = faixa(g.series.iter().flat_map(|s| s.valores.iter().copied()));
    let cor = r.cor_serie(g, 0);
    let claro = Cor::rgba(247, 249, 252, 255);
    let (cw, ch) = (plot.w / nc as f32, plot.h / nl as f32);
    let fmt = r.e.rotulos.formato.or(r.e.eixo_y.formato).unwrap_or_default();
    let mostra = r.e.rotulos.visivel.unwrap_or(true) && cw > 26.0 && ch > 14.0;
    let fv = Fonte::de(&r.base, &r.e.rotulos.fonte, 0.85);
    for (i, cat) in g.categorias.iter().enumerate() {
        d.texto(plot.x - 6.0, plot.y + ch * (i as f32 + 0.5) + fy.tamanho * 0.35, &cortar(cat, esq - 8.0, fy.tamanho), &fy, Ancora::Fim, 0.0, "rotulo-y");
    }
    for (j, s) in g.series.iter().enumerate() {
        for (i, v) in s.valores.iter().enumerate() {
            let t = if b > a { ((v - a) / (b - a)) as f32 } else { 0.5 };
            let c = crate::personalizacao::misturar(claro, cor, t.clamp(0.0, 1.0));
            let (x, y) = (plot.x + cw * j as f32, plot.y + ch * i as f32);
            let _ = writeln!(d.corpo, "<rect class=\"celula s{j} p{i}\" x=\"{x:.2}\" y=\"{y:.2}\" width=\"{:.2}\" height=\"{:.2}\" {} stroke=\"#ffffff\" stroke-width=\"1\"/>", cw, ch, preencher(c));
            if mostra {
                let mut fc = fv.clone();
                fc.cor = if t > 0.55 { Cor::rgba(255, 255, 255, 255) } else { Cor::rgba(32, 33, 36, 255) };
                d.texto(x + cw / 2.0, y + ch / 2.0 + fc.tamanho * 0.35, &cortar(&formatar(*v, fmt, casas_de(*v)), cw - 4.0, fc.tamanho), &fc, Ancora::Meio, 0.0, "rotulo");
            }
            d.mapa.pontos.push(AlvoPonto { serie: j, ponto: i, forma: FormaAlvo::Retangulo { x, y, largura: cw, altura: ch } });
        }
    }
    let maior = g.series.iter().map(|s| largura_texto(&s.nome, f.tamanho)).fold(0.0, f32::max);
    let passo = (((maior + 8.0) / cw.max(1.0)).ceil() as usize).max(1);
    for j in (0..nc).step_by(passo) {
        d.texto(plot.x + cw * (j as f32 + 0.5), plot.y + plot.h + f.tamanho * 1.15, &cortar(&g.series[j].nome, cw * passo as f32 - 4.0, f.tamanho), &f, Ancora::Meio, 0.0, "rotulo-x");
    }
}

fn linha_3d(g: &Grafico, d: &mut Desenho, r: &Resolvido<'_>, caixa: Ret, p: &PedidoAvancado) {
    let xs = p.xs(g);
    let zs = p.z.clone().unwrap_or_else(|| vec![0.0; xs.len()]);
    let (xa, xb) = faixa(xs.iter().copied());
    let (za, zb) = faixa(zs.iter().copied());
    let (ya, yb) = faixa(g.series.iter().enumerate().filter(|(i, _)| r.ativo(*i)).flat_map(|(_, s)| s.valores.iter().copied()));
    let norm = |v: f64, a: f64, b: f64| if b > a { ((v - a) / (b - a)) as f32 } else { 0.5 };
    let mp = &r.e.margens_plotagem;
    let area = caixa.encolher(mp.superior.unwrap_or(8.0), mp.inferior.unwrap_or(8.0), mp.esquerda.unwrap_or(8.0), mp.direita.unwrap_or(8.0));
    let proj = |x: f32, y: f32, z: f32| (area.x + area.w * (0.08 + 0.62 * x + 0.28 * z), area.y + area.h * (0.88 - 0.62 * y - 0.24 * z));
    // Caixa de referência: o "chão" e as três arestas dos eixos.
    let cor_grade = r.tema.map_or(GRADE_PADRAO, |t| t.grade.0);
    let cor_eixo = r.eixo_cor();
    let chao = [proj(0.0, 0.0, 0.0), proj(1.0, 0.0, 0.0), proj(1.0, 0.0, 1.0), proj(0.0, 0.0, 1.0)];
    let mut dd = caminho(&chao, false);
    dd.push_str(" Z");
    let _ = writeln!(d.corpo, "<path class=\"chao-3d\" d=\"{dd}\" {} {}/>", preencher(com_alfa(cor_grade, 0.35)), tracar(cor_grade, 1.0, Traco::Continuo));
    for k in 1..4 {
        let t = k as f32 / 4.0;
        let (a, b) = (proj(t, 0.0, 0.0), proj(t, 0.0, 1.0));
        d.linha(a.0, a.1, b.0, b.1, cor_grade, 0.8, Traco::Continuo, "grade-3d");
        let (a, b) = (proj(0.0, 0.0, t), proj(1.0, 0.0, t));
        d.linha(a.0, a.1, b.0, b.1, cor_grade, 0.8, Traco::Continuo, "grade-3d");
    }
    for (a, b) in [((0.0, 0.0, 0.0), (1.0, 0.0, 0.0)), ((0.0, 0.0, 0.0), (0.0, 1.0, 0.0)), ((0.0, 0.0, 0.0), (0.0, 0.0, 1.0))] {
        let (p0, p1) = (proj(a.0, a.1, a.2), proj(b.0, b.1, b.2));
        d.linha(p0.0, p0.1, p1.0, p1.1, cor_eixo, 1.2, Traco::Continuo, "eixo-3d");
    }
    let f = fonte_eixo(r, true);
    let fmt = |v: f64| formatar(v, FormatoNum::Automatico, casas_de(v));
    let (px, py, pz) = (proj(1.0, 0.0, 0.0), proj(0.0, 1.0, 0.0), proj(0.0, 0.0, 1.0));
    d.texto(px.0, px.1 + f.tamanho * 1.2, &format!("X {}–{}", fmt(xa), fmt(xb)), &f, Ancora::Fim, 0.0, "rotulo-x");
    d.texto(py.0 + 4.0, py.1 - 4.0, &format!("Y {}–{}", fmt(ya), fmt(yb)), &f, Ancora::Inicio, 0.0, "rotulo-y");
    d.texto(pz.0 + 4.0, pz.1, &format!("Z {}–{}", fmt(za), fmt(zb)), &f, Ancora::Inicio, 0.0, "rotulo-z");
    for (s, serie) in g.series.iter().enumerate().filter(|(s, _)| r.ativo(*s)) {
        let cor = r.cor_serie(g, s);
        let pts: Vec<(f32, f32)> = serie.valores.iter().enumerate()
            .map(|(i, y)| proj(norm(xs[i], xa, xb), norm(*y, ya, yb), norm(zs[i], za, zb))).collect();
        // Sombra no chão: ajuda a ler a profundidade.
        let sombra: Vec<(f32, f32)> = (0..pts.len()).map(|i| proj(norm(xs[i], xa, xb), 0.0, norm(zs[i], za, zb))).collect();
        let _ = writeln!(d.corpo, "<path class=\"sombra-3d s{s}\" d=\"{}\" fill=\"none\" {}/>", caminho(&sombra, false), tracar(com_alfa(cor, 0.25), 1.5, Traco::Tracejado));
        let _ = writeln!(d.corpo, "<path class=\"linha s{s}\" d=\"{}\" fill=\"none\" {} stroke-linejoin=\"round\"/>", caminho(&pts, false), tracar(cor, 2.4, Traco::Continuo));
        for (i, q) in pts.iter().enumerate() {
            marcador(d, FormaMarcador::Circulo, q.0, q.1, 5.0, cor, None);
            d.mapa.pontos.push(AlvoPonto { serie: s, ponto: i, forma: FormaAlvo::Circulo { x: q.0, y: q.1, raio: 7.0 } });
        }
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::Serie;

    fn g(series: usize) -> Grafico {
        Grafico {
            titulo: "Teste".into(),
            tipo: Tipo::Colunas,
            categorias: (1..=6).map(|i| format!("{i}")).collect(),
            series: (0..series).map(|s| Serie {
                nome: format!("S{s}"),
                valores: (0..6).map(|i| (10 + s * 5 + i * (s + 1)) as f64).collect(),
                cor: Cor::rgba(40, 100, 200, 255),
            }).collect(),
        }
    }

    #[test]
    fn os_nove_tipos_desenham_com_moldura_eixos_e_mapa() {
        let e = EstiloGrafico { tema: Some("oceano".into()), ..Default::default() };
        for (tipo, n) in [
            (TipoAvancado::Combinado, 3), (TipoAvancado::Histograma, 1), (TipoAvancado::Cascata, 1),
            (TipoAvancado::Radar, 2), (TipoAvancado::Bolhas, 2), (TipoAvancado::Velas, 4),
            (TipoAvancado::Gantt, 2), (TipoAvancado::MapaCalor, 3), (TipoAvancado::Linha3d, 2),
        ] {
            let mut gr = g(n);
            if tipo == TipoAvancado::Velas {
                // abertura, máxima, mínima, fechamento coerentes
                for i in 0..6 {
                    let a = gr.series[0].valores[i];
                    gr.series[1].valores[i] = a + 8.0;
                    gr.series[2].valores[i] = a - 6.0;
                    gr.series[3].valores[i] = a + if i % 2 == 0 { 3.0 } else { -3.0 };
                }
            }
            let mut p = PedidoAvancado::novo(tipo);
            if tipo == TipoAvancado::Linha3d {
                p.z = Some(vec![1.0, 3.0, 2.0, 5.0, 4.0, 6.0]);
            }
            let (svg, mapa) = gr.exportar_svg_avancado_com_mapa(560.0, 340.0, &e, &p).unwrap_or_else(|x| panic!("{tipo:?}: {x:?}"));
            assert!(svg.contains("class=\"titulo\"") && svg.contains(">Teste</text>"), "{tipo:?} sem título");
            assert!(svg.contains("#f2f8fb"), "{tipo:?} sem o fundo do tema");
            assert!(!mapa.pontos.is_empty(), "{tipo:?} sem pontos no mapa");
            assert!(!svg.contains("NaN"), "{tipo:?}");
            assert_eq!(svg.matches("<g").count(), svg.matches("</g>").count(), "{tipo:?}");
            assert_eq!(!mapa.legenda.is_empty(), tipo.tem_legenda(), "{tipo:?} legenda");
        }
    }

    #[test]
    fn histograma_cascata_e_gantt_dao_a_dica_propria() {
        let e = EstiloGrafico::default();
        let (_, m) = g(1).exportar_svg_avancado_com_mapa(500.0, 300.0, &e, &PedidoAvancado::novo(TipoAvancado::Histograma)).unwrap();
        let dica = m.dica.unwrap();
        assert_eq!(dica.series[0].1.iter().sum::<f64>(), 6.0, "as 6 amostras nas faixas");
        let (svg, m) = g(1).exportar_svg_avancado_com_mapa(500.0, 300.0, &e, &PedidoAvancado::novo(TipoAvancado::Cascata)).unwrap();
        assert!(svg.contains(">Total</text>"));
        assert_eq!(m.dica.unwrap().series[0].1.last().copied(), Some(g(1).series[0].valores.iter().sum()));
        let mut gt = g(2);
        gt.series[1].valores = gt.series[0].valores.iter().map(|v| v + 4.0).collect();
        let (_, m) = gt.exportar_svg_avancado_com_mapa(500.0, 300.0, &e, &PedidoAvancado::novo(TipoAvancado::Gantt)).unwrap();
        let alvo = m.pontos[0];
        let c = m.conteudo_dica(&gt, &alvo).unwrap();
        let nomes: Vec<&str> = c.linhas.iter().map(|l| l.rotulo.as_str()).collect();
        assert_eq!(nomes, ["Início", "Término", "Duração"]);
        assert_eq!(c.linhas[2].valor, 4.0);
        assert!(!c.linhas[0].data, "números pequenos não são datas");
        // Datas de verdade (número de série): dd/mm/aaaa e duração em dias.
        let mut gd = gt.clone();
        gd.series[0].valores = vec![46_000.0; 6];
        gd.series[1].valores = vec![46_010.0; 6];
        let (_, m) = gd.exportar_svg_avancado_com_mapa(500.0, 300.0, &e, &PedidoAvancado::novo(TipoAvancado::Gantt)).unwrap();
        let c = m.conteudo_dica(&gd, &m.pontos[0]).unwrap();
        assert!(c.linhas[0].data && c.linhas[1].data && !c.linhas[2].data);
        assert_eq!(c.linhas[2].rotulo, "Duração (dias)");
        assert_eq!(crate::formatado::data_de_serie(46_000.0), "09/12/2025");
        gt.series[1].valores[0] = 0.0;
        assert!(gt.exportar_svg_avancado_com_mapa(500.0, 300.0, &e, &PedidoAvancado::novo(TipoAvancado::Gantt)).is_err(), "término antes do início");
    }

    #[test]
    fn gantt_marca_hoje_e_pinta_pela_situacao() {
        // #133: concluída (fim < hoje), em andamento e futura; a pintada à mão vence.
        let mut gt = g(2);
        gt.categorias.truncate(4);
        gt.series[0].valores = vec![10.0, 18.0, 25.0, 1.0];
        gt.series[1].valores = vec![15.0, 22.0, 30.0, 5.0];
        let mut e = EstiloGrafico::default();
        let mut p = PedidoAvancado::novo(TipoAvancado::Gantt);
        p.hoje = Some((20.0, "Hoje".into()));
        let (sem, _) = gt.exportar_svg_avancado_com_mapa(500.0, 300.0, &e, &p).unwrap();
        assert!(!sem.contains("class=\"hoje\"") && !sem.contains("#43a047"), "desligado por padrão");
        e.gantt = crate::EstiloGantt { linha_hoje: true, pintar_situacao: true, ..Default::default() };
        e.series.entry(0).or_default().pontos.entry(3).or_default().cor = Some(crate::estilo::CorEstilo::rgb(255, 0, 255));
        let (svg, _) = gt.exportar_svg_avancado_com_mapa(500.0, 300.0, &e, &p).unwrap();
        assert!(svg.contains("class=\"hoje\"") && svg.contains(">Hoje</text>"), "linha de hoje");
        let barra = |i: usize| svg.lines().find(|l| l.contains(&format!("tarefa p{i}\""))).unwrap().to_owned();
        assert!(barra(0).contains("#43a047"), "concluída: {}", barra(0));
        assert!(barra(2).contains("#b0bec5"), "futura: {}", barra(2));
        assert!(!barra(1).contains("#43a047") && !barra(1).contains("#b0bec5"), "em andamento: cor da série");
        assert!(barra(3).contains("#ff00ff"), "pintada à mão vence: {}", barra(3));
        // Hoje fora do cronograma: sem linha.
        p.hoje = Some((90.0, "Hoje".into()));
        let (fora, _) = gt.exportar_svg_avancado_com_mapa(500.0, 300.0, &e, &p).unwrap();
        assert!(!fora.contains("class=\"hoje\""));
        // O estilo vai e volta pelo JSON.
        let json = serde_json::to_string(&e).unwrap();
        assert_eq!(serde_json::from_str::<EstiloGrafico>(&json).unwrap().gantt, e.gantt);
    }

    #[test]
    fn legenda_desliga_serie_no_combinado_e_no_radar() {
        let mut e = EstiloGrafico::default();
        e.legenda.alternar(1);
        for tipo in [TipoAvancado::Combinado, TipoAvancado::Radar] {
            let (svg, mapa) = g(3).exportar_svg_avancado_com_mapa(500.0, 300.0, &e, &PedidoAvancado::novo(tipo)).unwrap();
            assert!(!svg.contains("linha s1\""), "{tipo:?}");
            assert!(svg.contains("legenda-amostra desligado"), "{tipo:?}");
            assert!(!mapa.pontos.iter().any(|p| p.serie == 1), "{tipo:?}");
        }
    }
}
