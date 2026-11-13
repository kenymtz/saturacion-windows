//! Matriz de saturación. Función pura, sin dependencias de placa ni de UI.

/// Matriz 3×3 indexada `[fila][columna]`; salida = M × vector columna RGB.
pub type Matrix3 = [[f64; 3]; 3];

pub const IDENTITY: Matrix3 = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];

/// Pesos de luma BT.709.
pub const LUMA: [f64; 3] = [0.2126, 0.7152, 0.0722];

/// `M = s·I + (1 − s)·[LUMA; LUMA; LUMA]`.
/// `s = 1.0` deja la imagen igual, `0.0` la pasa a grises, `> 1.0` satura.
pub fn saturation_matrix(s: f64) -> Matrix3 {
    let mut m = [[0.0; 3]; 3];
    for (i, row) in m.iter_mut().enumerate() {
        for (j, cell) in row.iter_mut().enumerate() {
            *cell = (1.0 - s) * LUMA[j] + if i == j { s } else { 0.0 };
        }
    }
    m
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_close(actual: Matrix3, expected: Matrix3) {
        for i in 0..3 {
            for j in 0..3 {
                assert!(
                    (actual[i][j] - expected[i][j]).abs() < 1e-12,
                    "[{i}][{j}]: {} != {}",
                    actual[i][j],
                    expected[i][j]
                );
            }
        }
    }

    #[test]
    fn factor_one_is_identity() {
        assert_close(saturation_matrix(1.0), IDENTITY);
    }

    #[test]
    fn factor_zero_is_grayscale() {
        assert_close(saturation_matrix(0.0), [LUMA, LUMA, LUMA]);
    }

    #[test]
    fn rows_sum_to_one_so_grays_are_preserved() {
        for s in [0.0, 0.5, 1.0, 1.25, 3.0] {
            for row in saturation_matrix(s) {
                let sum: f64 = row.iter().sum();
                assert!((sum - 1.0).abs() < 1e-12, "s={s}: fila suma {sum}");
            }
        }
    }

    #[test]
    fn factor_three_boosts_diagonal_and_subtracts_other_channels() {
        let m = saturation_matrix(3.0);
        assert!((m[0][0] - 2.5748).abs() < 1e-12);
        assert!((m[0][1] - (-1.4304)).abs() < 1e-12);
        assert!((m[0][2] - (-0.1444)).abs() < 1e-12);
    }
}
