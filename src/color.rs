//! Ajustes de color: matriz de saturación y curvas de contraste y brillo. Funciones puras, sin dependencias de placa ni de UI.

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

/// Curva de contraste alrededor del gris medio: `y = clamp(0.5 + c·(x − 0.5), 0, 1)`.
/// `c = 1.0` deja la imagen igual; `> 1.0` oscurece sombras y aclara luces; `< 1.0` aplana.
pub fn contrast_curve(c: f64, x: f64) -> f64 {
    (0.5 + c * (x - 0.5)).clamp(0.0, 1.0)
}

/// Curva de brillo tipo gamma: `y = x^(1/b)`. `b = 1.0` deja la imagen igual; `> 1.0`
/// aclara medios y sombras; `< 1.0` los oscurece. El negro y el blanco no cambian.
pub fn brightness_curve(b: f64, x: f64) -> f64 {
    x.clamp(0.0, 1.0).powf(1.0 / b)
}

/// Curva de tono que se escribe en la placa: primero contraste, después brillo.
pub fn tone_curve(c: f64, b: f64, x: f64) -> f64 {
    brightness_curve(b, contrast_curve(c, x))
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

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-12
    }

    #[test]
    fn contrast_one_is_identity() {
        for x in [0.0, 0.1, 0.25, 0.5, 0.9, 1.0] {
            assert!(close(contrast_curve(1.0, x), x), "x={x}");
        }
    }

    #[test]
    fn contrast_keeps_mid_gray_fixed() {
        for c in [0.5, 1.0, 1.5, 2.0] {
            assert!(close(contrast_curve(c, 0.5), 0.5), "c={c}");
        }
    }

    #[test]
    fn contrast_curve_never_decreases() {
        for c in [0.5, 1.0, 1.5, 2.0] {
            let mut previous = contrast_curve(c, 0.0);
            for i in 1..=100 {
                let y = contrast_curve(c, f64::from(i) / 100.0);
                assert!(y >= previous, "c={c}, i={i}");
                previous = y;
            }
        }
    }

    #[test]
    fn contrast_output_stays_in_unit_range_and_clips() {
        for c in [0.5, 2.0, 4.0] {
            for i in 0..=100 {
                let y = contrast_curve(c, f64::from(i) / 100.0);
                assert!((0.0..=1.0).contains(&y), "c={c}, i={i}, y={y}");
            }
        }
        assert_eq!(contrast_curve(2.0, 0.0), 0.0);
        assert_eq!(contrast_curve(2.0, 1.0), 1.0);
        assert_eq!(contrast_curve(2.0, 0.1), 0.0);
    }

    #[test]
    fn half_contrast_flattens_toward_mid_gray() {
        assert!(close(contrast_curve(0.5, 0.0), 0.25));
        assert!(close(contrast_curve(0.5, 1.0), 0.75));
    }

    #[test]
    fn brightness_one_is_identity() {
        for x in [0.0, 0.1, 0.25, 0.5, 0.9, 1.0] {
            assert!(close(brightness_curve(1.0, x), x), "x={x}");
        }
    }

    #[test]
    fn brightness_keeps_black_and_white() {
        for b in [0.5, 1.0, 1.5, 2.0] {
            assert!(close(brightness_curve(b, 0.0), 0.0), "b={b}");
            assert!(close(brightness_curve(b, 1.0), 1.0), "b={b}");
        }
    }

    #[test]
    fn brightness_above_one_lightens_and_below_one_darkens() {
        for i in 0..=100 {
            let x = f64::from(i) / 100.0;
            assert!(brightness_curve(2.0, x) >= x - 1e-12, "x={x}");
            assert!(brightness_curve(0.5, x) <= x + 1e-12, "x={x}");
        }
    }

    #[test]
    fn brightness_curve_never_decreases() {
        for b in [0.5, 1.0, 1.5, 2.0] {
            let mut previous = brightness_curve(b, 0.0);
            for i in 1..=100 {
                let y = brightness_curve(b, f64::from(i) / 100.0);
                assert!(y >= previous, "b={b}, i={i}");
                previous = y;
            }
        }
    }

    #[test]
    fn brightness_reference_values() {
        assert!(close(brightness_curve(2.0, 0.25), 0.5));
        assert!(close(brightness_curve(0.5, 0.5), 0.25));
    }

    #[test]
    fn tone_curve_applies_contrast_then_brightness() {
        for i in 0..=20 {
            let x = f64::from(i) / 20.0;
            assert!(close(tone_curve(1.0, 1.0, x), x), "x={x}");
            for (c, b) in [(2.0, 0.5), (0.5, 2.0), (1.5, 1.5)] {
                let expected = brightness_curve(b, contrast_curve(c, x));
                assert!(close(tone_curve(c, b, x), expected), "c={c}, b={b}, x={x}");
            }
        }
    }
}
