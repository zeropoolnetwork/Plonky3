use alloc::vec;
use alloc::vec::Vec;
use itertools::Itertools;

use p3_air::{Air, TwoRowMatrixView};
use p3_challenger::{CanObserve, FieldChallenger};
use p3_commit::UnivariatePcs;
use p3_dft::reverse_slice_index_bits;
use p3_field::{AbstractExtensionField, AbstractField, Field, TwoAdicField};
use p3_matrix::dense::RowMajorMatrix;

use crate::{Proof, Config, Engine, to_values};

use p3_uni_stark::{VerificationError};

pub fn verify<C, E>(
    config: &C,
    challenger: &mut C::Challenger,
    proof: &Proof<C>,
    instance: RowMajorMatrix<C::Val>
) -> Result<(), VerificationError>
    where
        C: Config,
        E: Engine<F=C::Val, EF=C::Challenge>,
{
    let Proof {
        commitments,
        opened_values,
        opening_proof,
        multiset_sums,
        log_degree,
    } = proof;

    let log_degree = *log_degree as usize;
    let log_quotient_degree = E::LOG_QUOTIENT_DEGREE;
    let g_subgroup = C::Val::two_adic_generator(log_degree);

    challenger.observe(commitments.fixed.clone());
    challenger.observe(commitments.advice.clone());
    challenger.observe_slice(instance.values.as_slice());

    let gamma:C::Challenge = challenger.sample_ext_element();

    challenger.observe(commitments.multiset_f.clone());
    challenger.observe_slice(to_values::<C>(multiset_sums).as_slice());

    let alpha:C::Challenge = challenger.sample_ext_element();

    challenger.observe(commitments.quotient.clone());

    let zeta:C::Challenge = challenger.sample_ext_element();

    let local_and_next = [zeta, zeta * g_subgroup];

    let commits_and_points = &[
        (commitments.fixed.clone(), local_and_next.as_slice()),
        (commitments.advice.clone(), local_and_next.as_slice()),
        (commitments.multiset_f.clone(), local_and_next.as_slice()),
        (commitments.quotient.clone(), &[zeta.exp_power_of_2(log_quotient_degree)]),
    ];

    let values = vec![
        vec![vec![
            opened_values.fixed_local.clone(),
            opened_values.fixed_next.clone(),
        ]],
        vec![vec![
            opened_values.advice_local.clone(),
            opened_values.advice_next.clone(),
        ]],
        vec![vec![
            opened_values.multiset_f_local.clone(),
            opened_values.multiset_f_next.clone(),
        ]],
        vec![vec![opened_values.quotient.clone()]],
    ];

    config.pcs().verify_multi_batches(commits_and_points, values, opening_proof, challenger)
        .map_err(|_| VerificationError::InvalidOpeningArgument)?;


    let quotient: C::Challenge = {
        let mut parts = opened_values
            .quotient
            .chunks(C::Challenge::D)
            .map(|chunk| {
                chunk
                    .iter()
                    .enumerate()
                    .map(|(i, &c)| C::Challenge::monomial(i) * c)
                    .sum()
            }).collect::<Vec<C::Challenge>>();
        reverse_slice_index_bits(&mut parts);
        zeta.powers().zip(parts).map(|(zeta, part)| zeta * part).sum()
    };

    // TODO finalize verifier




    Ok(())
}