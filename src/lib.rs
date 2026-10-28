//! Dados e geometria de gráficos reutilizados por Sheet, Slide e Docs.
//! A Cena é independente de Iced/SVG/PDF; renderizadores desenham as mesmas
//! formas. Análise avançada/agrupamento continua no motor de dados do Sheet.

use feather_draw::{Cena, Cor};
#[cfg(test)]
use feather_draw::Figura;

pub mod motor;
pub mod analitico;
pub mod amostragem;
pub mod contrato;
pub mod aparencia;
pub mod eixos;
pub mod estilo;
pub mod estilos;
pub mod formatado;
pub mod filtro;
pub mod interacao;
pub mod pdf;
pub mod pizza;
pub mod svg;
pub mod temas;
pub mod tipografia;
pub mod personalizacao;
pub mod quadro;
pub mod modelo;
mod cena;
mod rotulos;
mod desenho;
mod moldura;
mod serde_cor;
pub use aparencia::{AparenciaGrafico, LayoutGrafico, Marcador};
// Modelo de estilo serializável do painel "Formatar gráfico" (vindo do featheroffice-charts).
// `estilo::temas` não é reexportado: na raiz, `temas` são as temáticas de cores.
pub use estilo::{EstiloGrafico, TemaGrafico};
pub use contrato::{AmostraDados, ContratoGrafico, MetadadosSerie};
pub use amostragem::{indices_lttb, ErroAmostragem};
pub use estilos::{Arredondamento, CantosArredondados, GradienteArea, Neon, PapelBarra, RaioBarra, Suavizacao};
pub use filtro::{FiltroLegenda, ItemLegenda};
pub use temas::{aplicar_tema, temas, Tema};
pub use eixos::{Eixos, EscalaLinear, Tick};
pub use modelo::{ErroModelo, ModeloGrafico};
pub use personalizacao::{
    clarear, misturar, ConfigEixo, ConfigEixos, ConteudoRotulo, CorPonto, EstiloBarras, EstiloMarcador,
    EstiloSerie, FonteTexto, FormatoRotuloNumerico, GradienteBarra, LinhaGrade, PosicaoRotulo, RegraCor,
    RotacaoRotulos, RotulosDados, TracoLinha,
};
pub use quadro::{
    AlinhamentoTitulo, Borda, Cartao, ConfigLegenda, EstiloPizza, EstiloTexto, Explosao, Fundo,
    PosicaoLegenda, Sombra,
};
pub use interacao::{AlvoGrafico, ErroInteracao, EventoGrafico, HitGrafico, InteracaoGrafico, MudancaInteracao, ViewportGrafico};
pub use pdf::ErroPdfGrafico;
pub use pizza::{FatiaPizza, RotuloPizza};
pub use svg::ErroSvgGrafico;
pub use tipografia::{AlinhamentoRotulo, CenaAnotada, ErroExportacaoTipografica, FormatoNumero, LocalidadeGrafico, RotuloGrafico, TipografiaGrafico};

/// Cores das fatias quando nem a série nem a paleta definem.
pub(crate) const CORES_PIZZA: [Cor; 5] = [
    Cor::rgb(43, 122, 120),
    Cor::rgb(11, 87, 208),
    Cor::rgb(232, 113, 10),
    Cor::rgb(24, 128, 56),
    Cor::rgb(142, 36, 170),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tipo {
    Colunas,
    Barras,
    Linhas,
    /// Área preenchida entre cada série e a linha do zero.
    Area,
    /// Linhas com segmentos horizontais seguidos de saltos verticais.
    Degraus,
    Pizza,
    Dispersao,
}
#[derive(Debug, Clone, PartialEq)]
pub struct Serie {
    pub nome: String,
    pub valores: Vec<f64>,
    pub cor: Cor,
}
#[derive(Debug, Clone, PartialEq)]
pub struct Grafico {
    pub titulo: String,
    pub tipo: Tipo,
    pub categorias: Vec<String>,
    pub series: Vec<Serie>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ErroGrafico {
    SemDados,
    TamanhosDiferentes,
    ValorInvalido,
    EixoInvalido,
}

impl Grafico {
    /// A pizza de [`Grafico::validar`] com várias séries (um anel cada), para a rosca.
    pub(crate) fn validar_rosca(&self) -> Result<(), ErroGrafico> {
        if self.categorias.is_empty() || self.series.is_empty() {
            return Err(ErroGrafico::SemDados);
        }
        if self.series.iter().any(|s| s.valores.len() != self.categorias.len()) {
            return Err(ErroGrafico::TamanhosDiferentes);
        }
        let valores = || self.series.iter().flat_map(|s| s.valores.iter());
        if valores().any(|v| !v.is_finite() || *v < 0.0) {
            return Err(ErroGrafico::ValorInvalido);
        }
        if !valores().any(|v| *v > 0.0) {
            return Err(ErroGrafico::SemDados);
        }
        Ok(())
    }
    pub fn validar(&self) -> Result<(), ErroGrafico> {
        if self.categorias.is_empty() || self.series.is_empty() {
            return Err(ErroGrafico::SemDados);
        }
        if self
            .series
            .iter()
            .any(|s| s.valores.len() != self.categorias.len())
        {
            return Err(ErroGrafico::TamanhosDiferentes);
        }
        if self
            .series
            .iter()
            .flat_map(|s| s.valores.iter())
            .any(|n| !n.is_finite())
        {
            return Err(ErroGrafico::ValorInvalido);
        }
        if self.tipo == Tipo::Pizza {
            if self.series.len() != 1 || self.series[0].valores.iter().any(|v| *v < 0.0) {
                return Err(ErroGrafico::ValorInvalido);
            }
            if !self.series[0].valores.iter().any(|v| *v > 0.0) {
                return Err(ErroGrafico::SemDados);
            }
        }
        if self.tipo == Tipo::Dispersao
            && self
                .categorias
                .iter()
                .any(|c| c.parse::<f64>().map_or(true, |x| !x.is_finite()))
        {
            return Err(ErroGrafico::EixoInvalido);
        }
        Ok(())
    }

    /// Mantém a renderização original e todas as amostras para compatibilidade.
    pub fn cena(&self, largura: f32, altura: f32) -> Result<Cena, ErroGrafico> {
        self.cena_impl(largura, altura, None, &AparenciaGrafico::default(), None, self)
    }

    /// Gera geometria com paleta, estilos, marcadores e margens configuráveis.
    /// O contrato `cena` legado permanece com aparência padrão.
    pub fn cena_estilizada(
        &self,
        largura: f32,
        altura: f32,
        aparencia: &AparenciaGrafico,
    ) -> Result<Cena, ErroGrafico> {
        self.cena_impl(largura, altura, None, aparencia, None, self)
    }

    /// Reduz apenas os pontos visuais de linhas por LTTB.
    /// As séries originais nunca são mutadas, reordenadas ou agregadas.
    /// Outros tipos mantêm sua representação completa.
    pub fn cena_amostrada(
        &self,
        largura: f32,
        altura: f32,
        max_pontos: usize,
    ) -> Result<Cena, ErroGrafico> {
        if max_pontos < 3 {
            return Err(ErroGrafico::ValorInvalido);
        }
        self.cena_impl(largura, altura, Some(max_pontos), &AparenciaGrafico::default(), None, self)
    }

    /// Amostragem visual sem alterar os dados, mantendo opções de aparência.
    pub fn cena_amostrada_estilizada(
        &self,
        largura: f32,
        altura: f32,
        max_pontos: usize,
        aparencia: &AparenciaGrafico,
    ) -> Result<Cena, ErroGrafico> {
        if max_pontos < 3 {
            return Err(ErroGrafico::ValorInvalido);
        }
        self.cena_impl(largura, altura, Some(max_pontos), aparencia, None, self)
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    fn exemplo(tipo: Tipo) -> Grafico {
        Grafico {
            titulo: "Receita".into(),
            tipo,
            categorias: vec!["1".into(), "2".into()],
            series: vec![Serie {
                nome: "Vendas".into(),
                valores: vec![10.0, 20.0],
                cor: Cor::rgb(11, 87, 208),
            }],
        }
    }
    #[test]
    fn personalizacao_paleta_contorno_marcadores_e_eixos_negativos() {
        let mut g = exemplo(Tipo::Barras);
        g.series[0].valores = vec![-10.0, 20.0];
        let cor = Cor::rgba(200, 50, 30, 128);
        let ap = AparenciaGrafico {
            paleta: vec![cor],
            opacidade: 0.5,
            cor_contorno: Some(Cor::rgb(1, 2, 3)),
            espessura_contorno: 1.0,
            ..Default::default()
        };
        let cena = g.cena_estilizada(600.0, 400.0, &ap).unwrap();
        let eixos = g.eixos_com_layout(600.0, 400.0, &ap.layout).unwrap().unwrap();
        let barras = cena.figuras.iter().filter_map(|figura| {
            if let Figura::Retangulo { area, estilo } = figura {
                assert_eq!(estilo.preenchimento, Some(Cor::rgba(200, 50, 30, 64)));
                assert_eq!(estilo.espessura, 1.0);
                Some(*area)
            } else {
                None
            }
        }).collect::<Vec<_>>();
        assert_eq!(barras.len(), 2);
        assert!((barras[0].x + barras[0].largura - eixos.zero).abs() < 0.01);
        assert!((barras[1].x - eixos.zero).abs() < 0.01);
        g.tipo = Tipo::Dispersao;
        let circulos = g.cena_estilizada(400.0, 300.0, &AparenciaGrafico {
            marcador: Marcador::Circulo,
            ..Default::default()
        }).unwrap();
        assert_eq!(circulos.figuras.iter().filter(|f| matches!(f, Figura::Elipse { .. })).count(), 2);
    }

    #[test]
    fn coluna_isolada_fica_centralizada_sem_encostar_nas_bordas() {
        let mut g = exemplo(Tipo::Colunas);
        g.categorias = vec!["b".into()];
        g.series[0].valores = vec![2.0];
        let cena = g.cena(800.0, 450.0).unwrap();
        let barras: Vec<_> = cena.figuras.iter().filter_map(|f| match f {
            Figura::Retangulo { area, estilo } if estilo.preenchimento.is_some() =>
                Some(*area),
            _ => None,
        }).collect();
        assert_eq!(barras.len(), 1);
        let eixo = g.eixos(800., 450.).unwrap().unwrap();
        let barra = barras[0];
        assert!(barra.largura <= 112.0, "a única barra não deve encher o plot");
        assert!(barra.x > eixo.area.x + 20.0, "deve respeitar margem esquerda");
        assert!(barra.x + barra.largura < eixo.area.x + eixo.area.largura - 20.0);
    }

    #[test]
    fn mesmos_dados_em_tres_tipos_de_cena() {
        for tipo in [
            Tipo::Colunas,
            Tipo::Linhas,
            Tipo::Pizza,
            Tipo::Barras,
            Tipo::Dispersao,
        ] {
            assert!(exemplo(tipo).cena(640.0, 480.0).unwrap().figuras.len() >= 2);
        }
    }
    #[test]
    fn cenas_de_graficos_exportam_svg_vetorial_reprodutivel() {
        for tipo in [
            Tipo::Colunas,
            Tipo::Linhas,
            Tipo::Pizza,
            Tipo::Barras,
            Tipo::Dispersao,
        ] {
            let cena = exemplo(tipo).cena(640.0, 480.0).unwrap();
            let svg = feather_draw::exportar_svg(&cena).unwrap();
            assert!(svg.starts_with("<svg xmlns=\"http://www.w3.org/2000/svg\""));
            assert!(svg.ends_with("</svg>\n"));
            assert_eq!(svg, feather_draw::exportar_svg(&cena).unwrap());
            assert_eq!(svg.matches("<path d=").count(), cena.figuras.len());
        }
    }

    #[test]
    fn rejeita_series_irregulares_e_nan() {
        let mut g = exemplo(Tipo::Colunas);
        g.series[0].valores.pop();
        assert_eq!(g.validar(), Err(ErroGrafico::TamanhosDiferentes));
        g.series[0].valores = vec![1.0, f64::NAN];
        assert_eq!(g.validar(), Err(ErroGrafico::ValorInvalido));
    }
    #[test]
    fn lttb_e_aplicado_ao_desenho_sem_modificar_series() {
        let mut g = exemplo(Tipo::Linhas);
        let n = 5_000;
        g.categorias = (0..n).map(|i| format!("T{i}")).collect();
        g.series[0].valores = (0..n)
            .map(|i| if i == 2_456 { 10_000.0 } else { 0.0 })
            .collect();
        let original = g.clone();
        let completa = g.cena(1024., 768.).unwrap();
        let reduzida = g.cena_amostrada(1024., 768., 48).unwrap();
        let pontos_da_serie = |cena: &Cena| match cena.figuras.last().unwrap() {
            Figura::Polilinha { pontos, .. } => pontos.clone(),
            _ => panic!("linha deve permanecer vetorial"),
        };
        let todos = pontos_da_serie(&completa);
        let menos = pontos_da_serie(&reduzida);
        assert_eq!(todos.len(), n);
        assert_eq!(menos.len(), 48);
        assert_eq!(todos.first(), menos.first());
        assert_eq!(todos.last(), menos.last());
        assert!(menos
            .iter()
            .any(|p| p.y == todos[2_456].y && p.x == todos[2_456].x));
        assert_eq!(g, original);
        assert_eq!(reduzida, g.cena_amostrada(1024., 768., 48).unwrap());
    }

    #[test]
    fn area_fecha_na_linha_zero_com_valores_positivos_e_negativos() {
        let mut g = exemplo(Tipo::Area);
        g.series[0].valores = vec![-8.0, 12.0];
        let eixos = g.eixos(600.0, 400.0).unwrap().unwrap();
        let cena = g.cena(600.0, 400.0).unwrap();
        match cena.figuras.last().unwrap() {
            Figura::Polilinha { pontos, fechada, estilo } => {
                assert!(*fechada);
                assert_eq!(pontos.len(), 4);
                assert!((pontos[0].y - eixos.zero).abs() < 0.001);
                assert!((pontos[3].y - eixos.zero).abs() < 0.001);
                assert!(pontos[1].y > eixos.zero);
                assert!(pontos[2].y < eixos.zero);
                assert!(estilo.preenchimento.is_some());
            }
            _ => panic!("area deve ser poligono vetorial"),
        }
        assert_eq!(g.series[0].valores, [-8.0, 12.0]);
        assert!(g.exportar_svg(600.0, 400.0).unwrap().contains("<path"));
    }

    #[test]
    fn degraus_horizontal_depois_vertical_sem_mudar_amostras() {
        let mut g = exemplo(Tipo::Degraus);
        g.categorias = vec!["A".into(), "B".into(), "C".into()];
        g.series[0].valores = vec![10.0, 30.0, -5.0];
        let cena = g.cena(640.0, 360.0).unwrap();
        match cena.figuras.last().unwrap() {
            Figura::Polilinha { pontos, fechada, estilo } => {
                assert!(!*fechada);
                assert_eq!(pontos.len(), 5);
                assert_eq!(pontos[0].y, pontos[1].y);
                assert_eq!(pontos[1].x, pontos[2].x);
                assert_eq!(pontos[2].y, pontos[3].y);
                assert_eq!(pontos[3].x, pontos[4].x);
                assert!(estilo.preenchimento.is_none());
            }
            _ => panic!("degraus devem ser segmentos vetoriais"),
        }
        assert_eq!(g.series[0].valores, [10.0, 30.0, -5.0]);
        assert_eq!(g.cena(640.0, 360.0).unwrap(), cena);
    }

    #[test]
    fn lttb_nao_muda_pizza_ou_agregacao_e_rejeita_limites_invalidos() {
        let pizza = exemplo(Tipo::Pizza);
        assert_eq!(
            pizza.cena(640., 480.).unwrap(),
            pizza.cena_amostrada(640., 480., 3).unwrap()
        );
        let barras = exemplo(Tipo::Barras);
        assert_eq!(
            barras.cena(640., 480.).unwrap(),
            barras.cena_amostrada(640., 480., 3).unwrap()
        );
        assert_eq!(
            exemplo(Tipo::Linhas).cena_amostrada(640., 480., 2),
            Err(ErroGrafico::ValorInvalido)
        );
    }
}
