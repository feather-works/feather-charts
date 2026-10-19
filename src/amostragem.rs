//! LTTB (Largest-Triangle-Three-Buckets) para desenhar séries longas.
//!
//! NÃO altera dados de origem, fórmulas, filtros ou agregações: retorna
//! somente índices a serem usados pelo renderizador de linhas.
//! O eixo X é o índice ordenado da categoria, mantendo o significado
//! temporal/ordinal do gráfico de linhas do FeatherWorks.

/// Falhas de entrada no downsampling de visualização.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErroAmostragem {
    LimiteInvalido,
    ValorInvalido,
}

/// Seleciona até `maximo` amostras e preserva sempre a primeira e a última.
///
/// - Entrada com 0, 1 ou 2 pontos não sofre alteração se couber no limite.
/// - Para reduzir séries de 3+ pontos, o limite mínimo é 3.
/// - Valores NaN e infinitos são rejeitados sem alterar a série.
/// - A seleção é determinística e os índices são estritamente crescentes.
/// - Complexidade temporal O(n), adicional O(maximo) (apenas índices).
/// - Normaliza Y para evitar overflow ao calcular áreas dos triângulos.
pub fn indices_lttb(valores: &[f64], maximo: usize) -> Result<Vec<usize>, ErroAmostragem> {
    if maximo == 0 {
        return Err(ErroAmostragem::LimiteInvalido);
    }
    if valores.iter().any(|v| !v.is_finite()) {
        return Err(ErroAmostragem::ValorInvalido);
    }
    let n = valores.len();
    if n <= maximo {
        return Ok((0..n).collect());
    }
    if maximo < 3 {
        return Err(ErroAmostragem::LimiteInvalido);
    }

    let max_abs = valores.iter().fold(0.0_f64, |max, v| max.max(v.abs()));
    let escala_y = if max_abs == 0.0 { 1.0 } else { max_abs };
    // Bucket fica entre as amostras fixas das extremidades.
    let passo = (n - 2) as f64 / (maximo - 2) as f64;
    let escala_x = (n - 1) as f64;
    let mut resultado = Vec::with_capacity(maximo);
    resultado.push(0);
    let mut anterior = 0usize;

    for bucket in 0..(maximo - 2) {
        let proximo_inicio = (((bucket + 1) as f64 * passo).floor() as usize + 1).min(n - 1);
        let proximo_fim = (((bucket + 2) as f64 * passo).floor() as usize + 1).min(n);
        let soma_y: f64 = valores[proximo_inicio..proximo_fim]
            .iter()
            .map(|y| *y / escala_y)
            .sum();
        let qtd = proximo_fim - proximo_inicio;
        let media_y = soma_y / qtd as f64;
        let media_x = ((proximo_inicio + proximo_fim - 1) as f64 / 2.0) / escala_x;

        let inicio = ((bucket as f64 * passo).floor() as usize + 1).min(n - 2);
        let fim = (((bucket + 1) as f64 * passo).floor() as usize + 1).min(n - 1);
        let ax = anterior as f64 / escala_x;
        let ay = valores[anterior] / escala_y;
        let mut area_max = f64::NEG_INFINITY;
        let mut escolhido = inicio;
        for (indice, valor) in valores.iter().enumerate().take(fim).skip(inicio) {
            let bx = indice as f64 / escala_x;
            let by = valor / escala_y;
            let area = ((ax - media_x) * (by - ay) - (ax - bx) * (media_y - ay)).abs();
            if area > area_max {
                area_max = area;
                escolhido = indice;
            }
        }
        resultado.push(escolhido);
        anterior = escolhido;
    }
    resultado.push(n - 1);
    debug_assert_eq!(resultado.len(), maximo);
    debug_assert!(resultado.windows(2).all(|par| par[0] < par[1]));
    Ok(resultado)
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn preserva_extremos_ordem_e_pico_isolado() {
        let mut valores = vec![0.0_f64; 1_000];
        valores[456] = 10_000.0;
        let indices = indices_lttb(&valores, 40).unwrap();
        assert_eq!(indices.len(), 40);
        assert_eq!(indices.first(), Some(&0));
        assert_eq!(indices.last(), Some(&999));
        assert!(indices.windows(2).all(|par| par[0] < par[1]));
        assert!(indices.contains(&456), "o pico não pode desaparecer");
        assert_eq!(indices, indices_lttb(&valores, 40).unwrap());
        assert_eq!(valores[456], 10_000.0);
    }

    #[test]
    fn nao_cria_pontos_artificiais_ou_subamostra_series_curtas() {
        assert_eq!(indices_lttb(&[], 1).unwrap(), Vec::<usize>::new());
        assert_eq!(indices_lttb(&[5.0], 1).unwrap(), vec![0]);
        assert_eq!(indices_lttb(&[0.0, 2.0], 2).unwrap(), vec![0, 1]);
        assert_eq!(indices_lttb(&[1.0, 2.0, 3.0], 3).unwrap(), vec![0, 1, 2]);
        assert_eq!(indices_lttb(&[1.0, 2.0, 3.0, 4.0], 3).unwrap().len(), 3);
    }

    #[test]
    fn limites_e_valores_invalidos_sao_rejeitados() {
        assert_eq!(indices_lttb(&[1.0], 0), Err(ErroAmostragem::LimiteInvalido));
        assert_eq!(
            indices_lttb(&[1.0, 2.0, 3.0], 2),
            Err(ErroAmostragem::LimiteInvalido)
        );
        assert_eq!(
            indices_lttb(&[1.0, f64::NAN], 3),
            Err(ErroAmostragem::ValorInvalido)
        );
        assert_eq!(
            indices_lttb(&[1.0, f64::INFINITY], 3),
            Err(ErroAmostragem::ValorInvalido)
        );
    }

    #[test]
    fn preserva_sinal_e_numeros_extremos_sem_overflow() {
        let mut valores = vec![-f64::MAX; 200];
        valores[101] = f64::MAX;
        let indices = indices_lttb(&valores, 16).unwrap();
        assert!(indices.contains(&101));
        assert_eq!(indices.len(), 16);
    }
}
