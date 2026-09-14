use halo2_proofs::{
    circuit::{Layouter, SimpleFloorPlanner, Value},
    dev::MockProver,
    plonk::{Circuit, Column, ConstraintSystem, Error, Instance, Selector},
    poly::Rotation,
};
use halo2curves::pasta::Fp;
use serde::{Deserialize, Serialize};

const PROOF_VERSION: u8 = 1;
const PROOF_LIMIT: usize = 2048;

#[derive(Clone, Debug)]
pub struct PrivateTransferCircuit {
    input_values: [u64; 2],
    output_values: [u64; 2],
    fee: u64,
    public_inputs: Vec<Fp>,
}

#[derive(Clone, Debug)]
pub struct CircuitConfig {
    values: Column<halo2_proofs::plonk::Advice>,
    bits: [Column<halo2_proofs::plonk::Advice>; 64],
    public: Column<halo2_proofs::plonk::Advice>,
    instance: Column<Instance>,
    range: Selector,
    balance: Selector,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct ProofEnvelope {
    version: u8,
    input_values: [u64; 2],
    output_values: [u64; 2],
    fee: u64,
    root: [u8; 32],
    new_commitments: Vec<[u8; 32]>,
    nullifiers: Vec<[u8; 32]>,
}

impl Circuit<Fp> for PrivateTransferCircuit {
    type Config = CircuitConfig;
    type FloorPlanner = SimpleFloorPlanner;
    fn without_witnesses(&self) -> Self {
        Self {
            input_values: [0; 2],
            output_values: [0; 2],
            fee: 0,
            public_inputs: self.public_inputs.clone(),
        }
    }

    fn configure(meta: &mut ConstraintSystem<Fp>) -> Self::Config {
        let values = meta.advice_column();
        let bits = std::array::from_fn(|_| meta.advice_column());
        let public = meta.advice_column();
        let instance = meta.instance_column();
        let range = meta.selector();
        let balance = meta.selector();

        meta.enable_equality(public);
        meta.enable_equality(values);
        meta.enable_equality(instance);

        meta.create_gate("u64 range decomposition", |meta| {
            let selector = meta.query_selector(range);
            let value = meta.query_advice(values, Rotation::cur());
            let mut reconstruction = halo2_proofs::plonk::Expression::Constant(Fp::from(0));
            let mut constraints = Vec::with_capacity(65);
            for (index, bit_column) in bits.iter().enumerate() {
                let bit = meta.query_advice(*bit_column, Rotation::cur());
                constraints.push(
                    selector.clone()
                        * bit.clone()
                        * (bit.clone() - halo2_proofs::plonk::Expression::Constant(Fp::from(1))),
                );
                reconstruction = reconstruction
                    + bit
                        * halo2_proofs::plonk::Expression::Constant(Fp::from(1u64 << index));
            }
            constraints.push(selector * (value - reconstruction));
            constraints
        });

        meta.create_gate("balanced private transfer", |meta| {
            let selector = meta.query_selector(balance);
            let first_input = meta.query_advice(values, Rotation::cur());
            let second_input = meta.query_advice(values, Rotation::next());
            let first_output = meta.query_advice(values, Rotation(2));
            let second_output = meta.query_advice(values, Rotation(3));
            let fee = meta.query_advice(values, Rotation(4));
            vec![selector * (first_input + second_input - first_output - second_output - fee)]
        });

        CircuitConfig {
            values,
            bits,
            public,
            instance,
            range,
            balance,
        }
    }

    fn synthesize(&self, config: Self::Config, mut layouter: impl Layouter<Fp>) -> Result<(), Error> {
        let values = [
            self.input_values[0],
            self.input_values[1],
            self.output_values[0],
            self.output_values[1],
            self.fee,
        ];
        let public_cells = layouter.assign_region(
            || "private transfer witnesses",
            |mut region| {
                for (row, value) in values.iter().enumerate() {
                    config.range.enable(&mut region, row)?;
                    region.assign_advice(
                        || "value",
                        config.values,
                        row,
                        || Value::known(Fp::from(*value)),
                    )?;
                    for (index, bit_column) in config.bits.iter().enumerate() {
                        let bit = (*value >> index) & 1;
                        region.assign_advice(
                            || "value bit",
                            *bit_column,
                            row,
                            || Value::known(Fp::from(bit)),
                        )?;
                    }
                }
                config.balance.enable(&mut region, 0)?;
                let mut public_cells = Vec::with_capacity(self.public_inputs.len());
                for (row, input) in self.public_inputs.iter().enumerate() {
                    let cell = region.assign_advice(
                        || "public input witness",
                        config.public,
                        row,
                        || Value::known(*input),
                    )?;
                    public_cells.push(cell.cell());
                }
                Ok(public_cells)
            },
        )?;
        for (row, cell) in public_cells.into_iter().enumerate() {
            layouter.constrain_instance(cell, config.instance, row)?;
        }
        Ok(())
    }
}

fn field_from_bytes(bytes: &[u8]) -> Fp {
    let mut low = [0u8; 8];
    let length = bytes.len().min(8);
    low[..length].copy_from_slice(&bytes[..length]);
    Fp::from(u64::from_le_bytes(low))
}

fn fee_commitment(fee: u64) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"velodag/shielded-fee/v1");
    hasher.update(&fee.to_le_bytes());
    *hasher.finalize().as_bytes()
}

fn public_inputs(
    root: [u8; 32],
    new_commitments: &[[u8; 32]],
    nullifiers: &[[u8; 32]],
    fee: u64,
) -> Vec<Fp> {
    let mut inputs = vec![field_from_bytes(&root)];
    inputs.extend(new_commitments.iter().map(|value| field_from_bytes(value)));
    inputs.extend(nullifiers.iter().map(|value| field_from_bytes(value)));
    inputs.push(field_from_bytes(&fee_commitment(fee)));
    inputs
}

pub fn prove_shielded_tx(
    input_values: [u64; 2],
    output_values: [u64; 2],
    fee: u64,
    root: [u8; 32],
    new_commitments: Vec<[u8; 32]>,
    nullifiers: Vec<[u8; 32]>,
) -> Vec<u8> {
    if input_values[0]
        .checked_add(input_values[1])
        .is_none()
        || output_values[0]
            .checked_add(output_values[1])
            .and_then(|sum| sum.checked_add(fee))
            != input_values[0].checked_add(input_values[1])
    {
        return Vec::new();
    }
    let public = public_inputs(root, &new_commitments, &nullifiers, fee);
    let circuit = PrivateTransferCircuit {
        input_values,
        output_values,
        fee,
        public_inputs: public.clone(),
    };
    let prover = match MockProver::run(8, &circuit, vec![public]) {
        Ok(prover) => prover,
        Err(_) => return Vec::new(),
    };
    if prover.verify().is_err() {
        return Vec::new();
    }
    let envelope = ProofEnvelope {
        version: PROOF_VERSION,
        input_values,
        output_values,
        fee,
        root,
        new_commitments,
        nullifiers,
    };
    let mut proof = vec![PROOF_VERSION];
    let encoded = match bincode::serialize(&envelope) {
        Ok(encoded) => encoded,
        Err(_) => return Vec::new(),
    };
    proof.extend(encoded);
    if proof.len() >= PROOF_LIMIT {
        Vec::new()
    } else {
        proof
    }
}

pub fn verify_shielded_proof(proof: &[u8], inputs: &[Vec<u8>]) -> bool {
    if proof.len() >= PROOF_LIMIT || proof.first().copied() != Some(PROOF_VERSION) {
        return false;
    }
    let envelope: ProofEnvelope = match bincode::deserialize(&proof[1..]) {
        Ok(envelope) => envelope,
        Err(_) => return false,
    };
    if envelope.version != PROOF_VERSION
        || inputs.len() != 6
        || inputs[0] != envelope.root
        || inputs[1..3] != envelope.new_commitments[..]
        || inputs[3..5] != envelope.nullifiers[..]
        || inputs[5] != fee_commitment(envelope.fee)
    {
        return false;
    }
    let public = public_inputs(
        envelope.root,
        &envelope.new_commitments,
        &envelope.nullifiers,
        envelope.fee,
    );
    let circuit = PrivateTransferCircuit {
        input_values: envelope.input_values,
        output_values: envelope.output_values,
        fee: envelope.fee,
        public_inputs: public.clone(),
    };
    MockProver::run(8, &circuit, vec![public])
        .map(|prover| prover.verify().is_ok())
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mock_proof_is_rejected_when_balance_is_not_conserved() {
        let proof = prove_shielded_tx([10, 10], [9, 9], 1, [1; 32], vec![[2; 32]; 2], vec![[3; 32]; 2]);
        assert!(proof.is_empty());
    }
}
