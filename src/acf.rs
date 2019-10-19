extern crate lapack_src;
extern crate lapack;

use num::Float;

use std::cmp;
use std::convert::From;
use std::ops::{Add, AddAssign, Div};
use std::result::Result;

use crate::ArimaError;

/// Calculate the auto-correlation function of a time series of length n.
///
/// # Arguments
///
/// * `&x` - Reference to input vector slice of length n.
/// * `max_lag` - Calculate ACF for this maximum lag. Defaults to n-1.
/// * `covariance` - If true, returns auto-covariances. If false, returns auto-correlations.
///
/// # Returns
///
/// * Output vector of length max_lag+1.
///
/// # Example
///
/// ```
/// use arima::acf;
/// let x = [1.0, 1.2, 1.4, 1.6];
/// acf::acf(&x, Some(2), false);
/// ```
pub fn acf<T: Float + From<u32> + From<f64> + Copy + Add + AddAssign + Div>(
    x: &[T],
    max_lag: Option<u32>,
    covariance: bool
) -> Result<Vec<T>, ArimaError> {
    let max_lag = match max_lag {
        // if upper bound for max_lag is n-1
        Some(max_lag) => cmp::min(max_lag as usize, x.len() - 1),
        None => x.len() - 1
    };
    let m = max_lag + 1;

    let len_x_usize = x.len();
    let len_x: T = From::from(len_x_usize as u32);
    let sum: T = From::from(0.0);

    let sum_x: T = x.iter().fold(sum, |sum, &xi| sum + xi);
    let mean_x: T = sum_x / len_x;

    //let mut y: Vec<T> = Vec::with_capacity(max_lag);
    let mut y: Vec<T> = vec![From::from(0.0); m];

    for t in 0..m {
        for i in 0..len_x_usize-t {
            let xi = x[i] - mean_x;
            let xi_t = x[i+t] - mean_x;
            y[t] = y[t] + (xi * xi_t) / len_x;
        }
        // we need y[0] to calculate the correlations, so we set it to 1.0 at the end
        if !covariance && t > 0 {
            y[t] = y[t] / y[0];
        }
    }
    if !covariance {
        y[0] = From::from(1.0);
    }
    Ok(y)
}

/// Calculate the auto-regressive coefficients of a time series of length n.
/// If you already calculated the auto-correlation coefficients (ACF), consider
/// using `ar_coef` instead.
///
/// # Arguments
///
/// * `&x` - Reference to input vector slice of length n.
/// * `order` - Order of the AR model.
///
/// # Returns
///
/// * Output vector of length order containing the AR coefficients.
///
/// # Example
///
/// ```
/// use arima::acf;
/// let x = [1.0, 1.2, 1.4, 1.6];
/// acf::ar_coef(&x, Some(2));
/// ```
pub fn ar_coef<T: Float + From<u32> + From<f64> + Into<f64> + Copy + AddAssign>(
    x: &[T],
    order: Option<u32>
) -> Result<Vec<T>, ArimaError> {
    let max_lag = match order {
        Some(order) => Some(order + 1),
        None => None
    };
    let rho = acf(&x, max_lag, false).unwrap();
    ar_coef_rho(&rho, order)
}

/// Calculate the auto-regressive coefficients of a time series of length n, given
/// the auto-correlation coefficients rho.
///
/// # Arguments
///
/// * `&rho` - Reference to auto-correlation coefficients rho.
/// * `order` - Order of the AR model.
///
/// # Returns
///
/// * Output vector of length order containing the AR coefficients.
///
/// # Example
///
/// ```
/// use arima::acf;
/// let x = [1.0, 1.2, 1.4, 1.6];
/// let rho = acf::acf(&x, None, false).unwrap();
/// acf::ar_coef_rho(&rho, Some(2));
/// ```
pub fn ar_coef_rho<T: Float + From<f64> + Into<f64> + Copy>(
    rho: &[T],
    order: Option<u32>
) -> Result<Vec<T>, ArimaError> {
    // phi_0 will be calculated separately
    let n = match order {
        Some(order) => cmp::min(order as usize, rho.len() - 1),
        None => rho.len() - 1
    };

    // we try to solve mr * x = r for x

    // build lower triangle matrix
    let mut mr: Vec<f64> = vec![1.0; n*n];

    for i in 0..n {
        for j in i+1..n {
            mr[i*n+j] = std::convert::Into::into(rho[j-i]);
        }
    }

    // build right hand vector rho_1..rho_n
    let mut b: Vec<f64> =vec![0.0; n];
    for i in 0..n {
        b[i] = std::convert::Into::into(rho[i+1]);
    }

    // build arguments to pass
    let mut info: i32 = 0;
    let ni = n as i32;

    // run lapack routine to solve symmetric positive-definite matrix system
    unsafe {
        lapack::dposv(b'L', ni,1, &mut mr, ni, &mut b, ni, &mut info);
    }

    if info != 0 {
        return Err(ArimaError);
    }

    // convert back to T
    let mut phi: Vec<T> = vec![From::from(0.0); n];
    for i in 0..n {
        phi[i] = std::convert::Into::into(b[i]);
    }
    Ok(phi)
}


/// Estimate the variance of a time series of length n.
/// If you already calculated the AR parameters, auto-correlation coefficients (ACF), and
/// the auto-covariance for lag zero, consider using `var_phi_rho_cov` instead.
///
/// # Arguments
///
/// * `&x` - Reference to input vector slice of length n.
/// * `order` - Order of the AR model. Defaults to n-1.
///
/// # Returns
///
/// * Estimated variance.
///
/// # Example
///
/// ```
/// use arima::acf;
/// let x = [1.0, 1.2, 1.4, 1.6];
/// acf::var(&x, Some(2));
/// ```
pub fn var<T: Float + From<u32> + From<f64> + Into<f64> + Copy + Add + AddAssign + Div>(
    x: &[T],
    order: Option<u32>
) -> Result<T, ArimaError> {
    let max_lag = match order {
        Some(order) => Some(order + 1),
        None => None
    };
    let rho = acf(&x, max_lag, false).unwrap();
    let phi = ar_coef_rho(&rho, order).unwrap();
    let cov0 = acf(&x, Some(0), true).unwrap()[0].clone();

    var_phi_rho_cov(&phi, &rho, cov0)
}

/// Estimate the variance of a time series of length n, given the AR parameters,
/// auto-correlation coefficients (ACF), and the auto-covariance for lag zero.
///
/// # Arguments
///
/// * `&x` - Reference to input vector slice of length n.
/// * `order` - Order of the AR model. Defaults to n-1.
///
/// # Returns
///
/// * Estimated variance.
///
/// # Example
///
/// ```
/// use arima::acf;
/// let x = [1.0, 1.2, 1.4, 1.6];
/// let rho = acf::acf(&x, Some(3), false).unwrap();
/// let phi = acf::ar_coef_rho(&rho, Some(2)).unwrap();
/// let cov0 = acf::acf(&x, Some(0), true).unwrap()[0].clone();
/// acf::var_phi_rho_cov(&phi, &rho, cov0);
/// ```
pub fn var_phi_rho_cov<T: Float + From<u32> + From<f64> + Copy + Add + AddAssign + Div>(
    phi: &[T],
    rho: &[T],
    cov0: T
) -> Result<T, ArimaError> {
    assert!(rho.len() > phi.len());

    let mut sum: T = From::from(0.0);
    for i in 0..phi.len() {
        sum += phi[i] * rho[i+1];
    }
    let one: T = From::from(1.0);
    Ok(cov0 * (one - sum))
}

/// Calculate the partial auto-correlation coefficients of a time series of length n.
/// If you already calculated the auto-correlation coefficients (ACF), consider
/// using `pacf_rho` instead.
///
/// # Arguments
///
/// * `&x` - Reference to input vector slice of length n.
/// * `max_lag` - Maximum lag to calculate the PACF for. Defaults to n.
///
/// # Returns
///
/// * Output vector of length `max_lag`.
///
/// # Example
///
/// ```
/// use arima::acf;
/// let x = [1.0, 1.2, 1.4, 1.6];
/// acf::pacf(&x, Some(2));
/// ```
pub fn pacf<T: Float + From<u32> + From<f64> + Into<f64> + Copy + AddAssign>(
    x: &[T],
    max_lag: Option<u32>
) -> Result<Vec<T>, ArimaError> {
    // get autocorrelations
    let rho = acf(&x, max_lag, false).unwrap();
    pacf_rho(&rho, max_lag)
}

/// Calculate the partial auto-correlation coefficients of a time series of length n, given
/// the auto-correlation coefficients rho.
///
/// # Arguments
///
/// * `&rho` - Reference to auto-correlation coefficients rho.
/// * `max_lag` - Maximum lag to calculate the PACF for. Defaults to n.
///
/// # Returns
///
/// * Output vector of length `max_lag`.
///
/// # Example
///
/// ```
/// use arima::acf;
/// let x = [1.0, 1.2, 1.4, 1.6];
/// let rho = acf::acf(&x, None, false).unwrap();
/// acf::pacf_rho(&rho, Some(2));
/// ```
pub fn pacf_rho<T: Float + From<u32> + From<f64> + Into<f64> + Copy + AddAssign>(
    rho: &[T],
    max_lag: Option<u32>
) -> Result<Vec<T>, ArimaError> {
    let max_lag = match max_lag {
        // if upper bound for max_lag is n-1
        Some(max_lag) => cmp::min(max_lag as usize, rho.len() - 1),
        None => rho.len() - 1
    };
    let m = max_lag + 1;

    // build output vector
    let mut y: Vec<T> = Vec::new();

    // calculate AR coefficients for each solution of order 1..max_lag
    for i in 1..m {
        let coef = ar_coef_rho(&rho, Some(i as u32));
        match coef {
            Ok(coef) => {
                // we now have a vector with i items, the last item is our partial correlation
                y.push(From::from(coef[i-1]));
            },
            Err(_) => {
                return Err(ArimaError);
            }
        }
    }
    Ok(y)
}