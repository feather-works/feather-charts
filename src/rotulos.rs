//! Rótulos de dados configuráveis (`AparenciaGrafico::rotulos`): conteúdo,
//! posição, formato pt-BR e fonte, em colunas, barras, linhas, áreas,
//! degraus, dispersão e pizza/rosca (com linha guia quando ficam fora).
use crate::cena::Desenho;
use crate::desenho::cor_contraste;
use crate::personalizacao::{PosicaoRotulo, RotulosDados};
use crate::tipografia::{AlinhamentoRotulo, RotuloGrafico};
use crate::{AparenciaGrafico, ErroGrafico, Grafico, Tipo};
use feather_draw::{Cena, Cor, Estilo, Figura, Ponto};

/// Acima disso o gráfico fica ilegível; nenhum rótulo é desenhado.
const MAXIMO_ROTULOS: usize = 400;

fn texto(r: &RotulosDados, serie: &str, categoria: &str, valor: f64, parte: Option<f64>) -> String {
    let mut partes: Vec<String> = Vec::with_capacity(4);
    if r.conteudo.serie && !serie.is_empty() {
        partes.push(serie.into());
    }
    if r.conteudo.categoria && !categoria.is_empty() {
        partes.push(categoria.into());
    }
    if r.conteudo.valor {
        partes.push(r.formato.formatar(valor));
    }
    if let Some(p) = parte.filter(|_| r.conteudo.porcentagem) {
        partes.push(r.porcentagem_texto(p));
    }
    partes.join(&r.conteudo.separador)
}

struct Fonte {
    tamanho: f32,
    cor: Option<Cor>,
    negrito: bool,
}

fn novo(t: String, x: f32, y: f32, f: &Fonte, cor: Cor, alinhamento: AlinhamentoRotulo) -> RotuloGrafico {
    RotuloGrafico { texto: t, x, y, tamanho: f.tamanho, cor: f.cor.unwrap_or(cor), alinhamento, negrito: f.negrito, rotacao: 0.0 }
}

/// Rótulos padrão da pizza no layout novo: só a categoria, fora, com linha guia.
pub(crate) fn padrao_pizza() -> RotulosDados {
    let mut r = RotulosDados::default();
    r.conteudo.valor = false;
    r.conteudo.categoria = true;
    r
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn rotulos_dados(
    vis: &Grafico,
    d: &Desenho,
    ap: &AparenciaGrafico,
    mapa: &[usize],
    r: &RotulosDados,
    fonte_base: f32,
    cor_texto: Cor,
    cena: &mut Cena,
    saida: &mut Vec<RotuloGrafico>,
) -> Result<(), ErroGrafico> {
    let f = Fonte { tamanho: r.fonte.tamanho.unwrap_or(fonte_base), cor: r.fonte.cor, negrito: r.fonte.negrito };
    let ts = f.tamanho;
    let ativo = |serie_original: usize| ap.estilo_serie(serie_original).and_then(|e| e.rotulos).unwrap_or(r.ativo);
    let extremos = |v: &[f64]| v.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), x| (a.min(*x), b.max(*x)));

    if let Some(geo) = &d.pizza {
        if !ativo(0) {
            return Ok(());
        }
        let fatias = vis.fatias_pizza(d.cena.largura, d.cena.altura)?;
        if fatias.len() > MAXIMO_ROTULOS {
            return Ok(());
        }
        let (vmin, vmax) = extremos(&vis.series[0].valores);
        let mut fora: Vec<(bool, f32, Ponto, Ponto, String)> = Vec::new();
        for fatia in &fatias {
            let original = mapa[fatia.indice];
            let t = texto(r, &vis.series[0].nome, &fatia.categoria, fatia.valor, Some(fatia.proporcao));
            if t.is_empty() {
                continue;
            }
            let meio = ((fatia.inicio + fatia.fim) / 2.0 + geo.giro) as f32;
            let (dx, dy) = geo.deslocamento(original, meio);
            let c = Ponto::novo(geo.centro.x + dx, geo.centro.y + dy);
            let dir = (meio.cos(), meio.sin());
            if r.posicao == PosicaoRotulo::Fora {
                let borda = Ponto::novo(c.x + geo.raio * dir.0, c.y + geo.raio * dir.1);
                let cotovelo = Ponto::novo(c.x + (geo.raio + 12.0) * dir.0, c.y + (geo.raio + 12.0) * dir.1);
                fora.push((dir.0 >= 0.0, cotovelo.y, borda, cotovelo, t));
            } else {
                let fr = match r.posicao {
                    PosicaoRotulo::PontaDentro => 0.8,
                    PosicaoRotulo::Base => 0.32,
                    _ => 0.58,
                };
                let rr = geo.furo + (geo.raio - geo.furo) * fr;
                let cor = cor_contraste(ap.cor_fatia(original, fatia.valor, vmin, vmax));
                saida.push(novo(t, c.x + rr * dir.0, c.y + rr * dir.1 + ts * 0.35, &f, cor, AlinhamentoRotulo::Centro));
            }
        }
        // Fora: separa os lados e empurra para baixo os que encostariam.
        for direita in [false, true] {
            let mut lado: Vec<_> = fora.iter().filter(|x| x.0 == direita).collect();
            lado.sort_by(|a, b| a.1.total_cmp(&b.1));
            let mut proximo = f32::NEG_INFINITY;
            for (_, y, borda, cotovelo, t) in lado {
                // O texto não sai do quadro do gráfico (antes, "Restauração Florestal"
                // à esquerda de uma rosca estreita saía cortado): encosta na borda.
                let y = y.max(proximo).clamp(ts, (d.cena.altura - ts * 0.5).max(ts));
                proximo = y + ts * 1.25;
                let w = crate::moldura::largura_texto(t, ts);
                let fim_x = if direita {
                    (cotovelo.x + 10.0).min(d.cena.largura - w - 5.0)
                } else {
                    (cotovelo.x - 10.0).max(w + 5.0)
                };
                if r.linha_guia {
                    let guia = Cor::rgba(cor_texto.r, cor_texto.g, cor_texto.b, 150);
                    cena.adicionar(Figura::Polilinha {
                        pontos: vec![*borda, Ponto::novo(cotovelo.x, y), Ponto::novo(fim_x, y)],
                        fechada: false,
                        estilo: Estilo { preenchimento: None, contorno: Some(guia), espessura: 1.0 },
                    });
                }
                let (x, al) = if direita {
                    (fim_x + 3.0, AlinhamentoRotulo::Inicio)
                } else {
                    (fim_x - 3.0, AlinhamentoRotulo::Fim)
                };
                saida.push(novo(t.clone(), x, y + ts * 0.35, &f, cor_texto, al));
            }
        }
        return Ok(());
    }

    let Some(eixos) = &d.eixos else { return Ok(()) };
    let total: usize = vis.series.len() * vis.categorias.len();
    if total > MAXIMO_ROTULOS {
        return Ok(());
    }
    // Parte de cada valor no total (absoluto) da categoria.
    let soma: Vec<f64> = (0..vis.categorias.len())
        .map(|i| vis.series.iter().map(|s| s.valores[i].abs()).sum())
        .collect();
    let parte = |i: usize, v: f64| if soma[i] > 0.0 { Some(v / soma[i]) } else { None };
    let invertido = ap.eixos.valores.invertido;
    match vis.tipo {
        Tipo::Colunas | Tipo::Barras => {
            let vertical = vis.tipo == Tipo::Colunas;
            for (s, i, a) in vis.retangulos_barras_em(eixos, ap)? {
                let so = mapa[s];
                if !ativo(so) {
                    continue;
                }
                let serie = &vis.series[s];
                let v = serie.valores[i];
                let t = texto(r, &serie.nome, &vis.categorias[i], v, parte(i, v));
                if t.is_empty() {
                    continue;
                }
                let (lo, hi) = extremos(&serie.valores);
                let preench = ap.cor_ponto(so, i, v, ap.cor_serie(so, serie.cor), lo, hi);
                let dentro = cor_contraste(preench);
                let positivo = (v >= 0.0) != invertido;
                let (cx, cy) = (a.x + a.largura / 2.0, a.y + a.altura / 2.0);
                let (x, y, al, cor) = if vertical {
                    let (topo, base) = (a.y, a.y + a.altura);
                    // Junto à borda de cima (baseline abaixo dela) ou de baixo.
                    let (em_cima, em_baixo) = (topo + ts + 2.0, base - 4.0);
                    let c = AlinhamentoRotulo::Centro;
                    match (r.posicao, positivo) {
                        (PosicaoRotulo::Fora, true) => (cx, (topo - 3.0).max(ts), c, cor_texto),
                        (PosicaoRotulo::Fora, false) => (cx, base + ts + 2.0, c, cor_texto),
                        (PosicaoRotulo::PontaDentro, true) | (PosicaoRotulo::Base, false) => (cx, em_cima, c, dentro),
                        (PosicaoRotulo::PontaDentro, false) | (PosicaoRotulo::Base, true) => (cx, em_baixo, c, dentro),
                        (PosicaoRotulo::Centro, _) => (cx, cy + ts * 0.35, c, dentro),
                    }
                } else {
                    let (esq, dir) = (a.x, a.x + a.largura);
                    let base_y = cy + ts * 0.35;
                    match (r.posicao, positivo) {
                        (PosicaoRotulo::Fora, true) => (dir + 3.0, base_y, AlinhamentoRotulo::Inicio, cor_texto),
                        (PosicaoRotulo::Fora, false) => (esq - 3.0, base_y, AlinhamentoRotulo::Fim, cor_texto),
                        (PosicaoRotulo::PontaDentro, true) => (dir - 4.0, base_y, AlinhamentoRotulo::Fim, dentro),
                        (PosicaoRotulo::PontaDentro, false) => (esq + 4.0, base_y, AlinhamentoRotulo::Inicio, dentro),
                        (PosicaoRotulo::Centro, _) => (cx, base_y, AlinhamentoRotulo::Centro, dentro),
                        (PosicaoRotulo::Base, true) => (esq + 4.0, base_y, AlinhamentoRotulo::Inicio, dentro),
                        (PosicaoRotulo::Base, false) => (dir - 4.0, base_y, AlinhamentoRotulo::Fim, dentro),
                    }
                };
                saida.push(novo(t, x, y, &f, cor, al));
            }
        }
        Tipo::Linhas | Tipo::Area | Tipo::Degraus | Tipo::Dispersao => {
            let escala = eixos.escala_y.ok_or(ErroGrafico::EixoInvalido)?;
            let a = eixos.area;
            let xs = vis.xs_pontos(a, ap)?;
            let folga = ap.tamanho_marcador / 2.0 + 4.0;
            for (s, serie) in vis.series.iter().enumerate() {
                let so = mapa[s];
                if !ativo(so) {
                    continue;
                }
                for (i, v) in serie.valores.iter().enumerate() {
                    let t = texto(r, &serie.nome, &vis.categorias[i], *v, parte(i, *v));
                    if t.is_empty() {
                        continue;
                    }
                    let y = a.y + a.altura * (1.0 - escala.fracao(*v));
                    let y = match r.posicao {
                        PosicaoRotulo::Fora => y - folga,
                        PosicaoRotulo::Base => y + folga + ts * 0.8,
                        PosicaoRotulo::Centro | PosicaoRotulo::PontaDentro => y + ts * 0.35,
                    };
                    saida.push(novo(t, xs[i], y, &f, cor_texto, AlinhamentoRotulo::Centro));
                }
            }
        }
        Tipo::Pizza => {}
    }
    Ok(())
}
