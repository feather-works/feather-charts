//! CHT-018: gera SVG real dos 25 gráficos a partir de uma única API Rust.
//! cargo run \
//!   --example cht018_25_tipos -j 2 -- /tmp/cht018-25-tipos
use feather_charts::{
    motor::{renderizar_svg, Entrada, TipoGrafico}, Serie,
};
use feather_draw::Cor;
use std::{error::Error, path::PathBuf};

fn entrada(tipo: TipoGrafico) -> Entrada {
    let categorias = (1..=12).map(|i| i.to_string()).collect();
    let mk = |nome:&str, valores:Vec<f64>, cor| Serie {
        nome:nome.to_string(), valores, cor,
    };
    let a = vec![3., 5., 2., 8., 6., 4., 9., 10., 6., 8., 5., 7.];
    let b: Vec<f64> = a.iter().map(|v|v+2.).collect();
    let c = a.iter().map(|v|v-1.).collect();
    let d = a.iter().map(|v|v+1.).collect();
    let series = match tipo {
        TipoGrafico::Candlestick => vec![
            mk("Abertura",a.clone(),Cor::rgb(25,150,100)),
            mk("Máxima",b.clone(),Cor::rgb(50,130,160)),
            mk("Mínima",c,Cor::rgb(200,70,70)),
            mk("Fechamento",d,Cor::rgb(90,110,220)),
        ],
        TipoGrafico::Gantt => vec![
            mk("Início",a.clone(),Cor::rgb(50,130,220)),
            mk("Fim",b.clone(),Cor::rgb(90,160,100)),
        ],
        _ => vec![
            mk("A",a.clone(),Cor::rgb(47,112,218)),
            mk("B",b.clone(),Cor::rgb(221,75,56)),
            mk("C",d,Cor::rgb(62,167,112)),
        ],
    };
    Entrada {
        titulo:format!("FeatherWorks: {}",tipo.id()),
        categorias,
        series,
        x_values:Some((1..=12).map(f64::from).collect()),
        z_values:Some((1..=12).map(|i|f64::from((i*7)%13)).collect()),
    }
}
fn main() -> Result<(),Box<dyn Error>> {
    let dir=std::env::args_os().nth(1).map(PathBuf::from)
        .unwrap_or_else(||PathBuf::from("/tmp/cht018-25-tipos"));
    std::fs::create_dir_all(&dir)?;
    let mut html=String::from("<!doctype html><html lang='pt-br'><meta charset='utf-8'><title>FeatherWorks Charts 25 tipos</title><style>body{font-family:sans-serif;background:#f3f4f6}main{display:grid;grid-template-columns:repeat(auto-fill,minmax(360px,1fr));gap:12px}figure{padding:10px;background:white;border-radius:8px;margin:0}img{width:100%}</style><h1>Charts Rust — 25 tipos</h1><main>");
    for tipo in TipoGrafico::TODOS {
        let entrada=entrada(tipo);
        let svg=renderizar_svg(tipo,&entrada,640.,380.)
            .map_err(|e|format!("{}: {e:?}",tipo.id()))?;
        let nome=format!("{}.svg",tipo.id());
        std::fs::write(dir.join(&nome),svg.as_bytes())?;
        html.push_str(&format!("<figure><figcaption>{}</figcaption><img src='{}' /></figure>",tipo.id(),nome));
    }
    html.push_str("</main></html>");
    std::fs::write(dir.join("index.html"),html)?;
    println!("CHT-018: 25 SVGs gerados em {}",dir.display());
    println!("Inspecione index.html e compare com chart.ts antes de retirar o renderer legado.");
    Ok(())
}
