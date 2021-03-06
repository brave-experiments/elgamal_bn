#![allow(non_snake_case)]
extern crate rand;

extern crate bincode;
extern crate rustc_serialize;

use bn::*;
use crate::errors::{ConversionError, ProofError};

use rand::thread_rng;
use sha3::{Digest, Keccak256};

use crate::ciphertext::*;
use bincode::SizeLimit::Infinite;
use bincode::rustc_serialize::{encode, decode};
use rustc_serialize::{Encodable, Decodable};
use rustc_serialize::hex::{FromHex, ToHex};


/// The `PublicKey` struct represents an ElGamal public key.
#[derive(Copy, Clone, Debug)]
pub struct PublicKey(G1);

impl PublicKey {
    /// Encrypts a message in the Ristretto group. It has the additive homomorphic property,
    /// allowing addition (and subtraction) by another ciphertext and multiplication (and division)
    /// by scalars.
    ///
    /// #Example
    /// ```
    /// extern crate rand;
    /// use elgamal_bn::public::{PublicKey, };
    /// use elgamal_bn::private::{SecretKey, };
    /// use bn::{Fr, G1, Group};
    ///
    /// # fn main() {
    ///        let mut csprng = rand::thread_rng();
    ///        // Generate key pair
    ///        let sk = SecretKey::new(&mut csprng);
    ///        let pk = PublicKey::from(&sk);
    ///
    ///        // Generate random messages
    ///        let ptxt1 = G1::random(&mut csprng);
    ///        let ptxt2 = G1::random(&mut csprng);
    ///
    ///        // Encrypt messages
    ///        let ctxt1 = pk.encrypt(&ptxt1);
    ///        let ctxt2 = pk.encrypt(&ptxt2);
    ///
    ///        // Add ciphertexts and check that addition is maintained in the plaintexts
    ///        let encrypted_addition = ctxt1 + ctxt2;
    ///        let decrypted_addition = sk.decrypt(&encrypted_addition);
    ///
    ///        assert_eq!(ptxt1 + ptxt2, decrypted_addition);
    ///
    ///        // Multiply by scalar and check that multiplication is maintained in the plaintext
    ///        let scalar_mult = Fr::random(&mut csprng);
    ///        assert_eq!(sk.decrypt(&(ctxt1 * scalar_mult)), ptxt1 * scalar_mult);
    /// # }
    /// ```
    pub fn encrypt(self, message: &G1) -> Ciphertext {
        let rng = &mut thread_rng();
        let random: Fr = Fr::random(rng);

        let random_generator = G1::one() * random;
        let encrypted_plaintext = *message + self.0 * random;
        Ciphertext {
            pk: self,
            points: (random_generator, encrypted_plaintext),
        }
    }

    /// Get the public key point
    pub fn get_point(&self) -> G1 {
        self.0
    }

    /// Get the public key point as an Affine point
    pub fn get_point_affine(&self) -> AffineG1 {
        AffineG1::from_jacobian(self.0).unwrap()
    }

    /// Get the public key point as a string
    pub fn get_point_hex_string(&self) -> Result<(String, String), ConversionError> {
        get_point_as_hex_str(self.0)
    }

    /// This function is only defined for testing purposes for the
    /// `prove_correct_decryption_no_Merlin`. Verification should
    /// happen in `Solidity`.
    /// Example
    /// ```
    /// extern crate rand;
    /// use elgamal_bn::public::{PublicKey, };
    /// use elgamal_bn::private::{SecretKey, };
    /// use bn::{G1, Group};
    ///
    /// # fn main() {
    ///    let mut csprng = rand::thread_rng();
    ///    let sk = SecretKey::new(&mut csprng);
    ///    let pk = PublicKey::from(&sk);
    ///
    ///    let plaintext = G1::random(&mut csprng);
    ///    let ciphertext = pk.encrypt(&plaintext);
    ///
    ///    let decryption = sk.decrypt(&ciphertext);
    ///    let proof = sk.prove_correct_decryption_no_Merlin(&ciphertext, &decryption).unwrap();
    ///
    ///    assert!(pk.verify_correct_decryption_no_Merlin(proof, ciphertext, decryption).is_ok());
    /// # }
    /// ```
    pub fn verify_correct_decryption_no_Merlin(
        self,
        proof: ((G1, G1), Fr),
        ciphertext: Ciphertext,
        message: G1,
    ) -> Result<(), ProofError> {
        let ((announcement_base_G, announcement_base_ctxtp0), response) = proof;

        let message_affine = AffineG1::from_jacobian(message).ok_or(ConversionError::AffineConversionFailure)?;
        let ctx1_affine = AffineG1::from_jacobian(ciphertext.points.0).ok_or(ConversionError::AffineConversionFailure)?;
        let ctx2_affine = AffineG1::from_jacobian(ciphertext.points.1).ok_or(ConversionError::AffineConversionFailure)?;
        let announcement_g_affine = AffineG1::from_jacobian(announcement_base_G).ok_or(ConversionError::AffineConversionFailure)?;
        let announcement_ctxt0_affine = AffineG1::from_jacobian(announcement_base_ctxtp0).ok_or(ConversionError::AffineConversionFailure)?;
        let generator_affine = AffineG1::from_jacobian(G1::one()).ok_or(ConversionError::AffineConversionFailure)?;
        let pk_affine = AffineG1::from_jacobian(self.get_point()).ok_or(ConversionError::AffineConversionFailure)?;

        let hash = Keccak256::new()
            .chain(encode(&message_affine, Infinite).unwrap())
            .chain(encode(&ctx1_affine, Infinite).unwrap())
            .chain(encode(&ctx2_affine, Infinite).unwrap())
            .chain(encode(&announcement_g_affine, Infinite).unwrap())
            .chain(encode(&announcement_ctxt0_affine, Infinite).unwrap())
            .chain(encode(&generator_affine, Infinite).unwrap())
            .chain(encode(&pk_affine, Infinite).unwrap())
        ;

        let challenge = Fr::from_slice(&hash.result()[..]).unwrap();

        if !(G1::one() * response == announcement_base_G + self.get_point() * challenge
            && ciphertext.points.0 * response
                == announcement_base_ctxtp0 + (ciphertext.points.1 - message) * challenge) {
            return Err(ProofError::VerificationError);
        }
        Ok(())
    }

    pub fn from_hex_string(hex_coords: (String, String)) -> Result<Self, ConversionError> {
        if &hex_coords.0[0..2] != "0x" || &hex_coords.1[0..2] != "0x" {
            return Err(ConversionError::IncorrectHexString);
        }

        if hex_coords.0.len() != 66 || hex_coords.1.len() != 66 {
            return Err(ConversionError::InvalidHexLength);
        }

        let combined_string = "04".to_owned() + &hex_coords.0[2..] + &hex_coords.1[2..];
        let pk_point: G1 = from_hex(&combined_string)?;
        Ok(PublicKey::from(pk_point))
    }
}

// outputs a point in hex format '0x...'
pub fn get_point_as_hex_str(point: G1) -> Result<(String, String), ConversionError> {
    let hex_point = into_hex(point).ok_or(ConversionError::InvalidHexConversion)?;
    let sol_hex_x = "0x".to_owned() + &hex_point[2..66];
    let sol_hex_y = "0x".to_owned() + &hex_point[66..];
    Ok((sol_hex_x, sol_hex_y))
}

// outputs a scalar in hex format '0x...'
pub fn get_scalar_as_hex_str(scalar: Fr) -> Result<String, ConversionError> {
    let hex_scalar = into_hex(scalar).ok_or(ConversionError::InvalidHexConversion)?;
    let sol_hex_scalar = "0x".to_owned() + &hex_scalar;
    Ok(sol_hex_scalar)
}

pub fn get_fq_as_hex_str(scalar: Fq) -> Result<String, ConversionError> {
    let hex_scalar = into_hex(scalar).ok_or(ConversionError::InvalidHexConversion)?;
    let sol_hex_scalar = "0x".to_owned() + &hex_scalar;
    Ok(sol_hex_scalar)
}

impl From<G1> for PublicKey {
    /// Given a secret key, compute its corresponding Public key
    fn from(point: G1) -> PublicKey {
        PublicKey(point)
    }
}

impl PartialEq for PublicKey {
    fn eq(&self, other: &PublicKey) -> bool {
        self.0 == other.0
    }
}

pub fn into_hex<S: Encodable>(obj: S) -> Option<String> {
    encode(&obj, Infinite).ok().map(|e| e.to_hex())
}

pub fn from_hex<S: Decodable>(s: &str) -> Result<S, ConversionError> {
    let s = s.from_hex()?;
    let d = decode(&s)?;
    Ok(d)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::thread_rng;
    use crate::private::SecretKey;

    #[test]
    fn test_hex_string_conversion() {
        let pk = PublicKey::from(G1::one() + G1::one());
        let pk_string = pk.get_point_hex_string().unwrap();
        assert_eq!(pk_string.0, "0x030644e72e131a029b85045b68181585d97816a916871ca8d3c208c16d87cfd3");
        assert_eq!(pk_string.1, "0x15ed738c0e0a7c92e7845f96b2ae9c0a68a6a449e3538fc7ff3ebf7a5a18a2c4");
    }

    #[test]
    fn test_from_hex_conversion() {
        let sk = SecretKey::new(&mut thread_rng());
        let pk = PublicKey::from(&sk);
        let pk_hex = pk.get_point_hex_string().unwrap();

        let pk_from_hex = PublicKey::from_hex_string(pk_hex).unwrap();
        assert_eq!(pk, pk_from_hex);

        let scalar = Fr::one();
        let scalar_hex = get_scalar_as_hex_str(scalar);
        assert_eq!(scalar_hex.unwrap(), "0x0000000000000000000000000000000000000000000000000000000000000001");
    }

    #[test]
    fn test_failure_from_hex_conversion() {
        let hex_coords: (String, String) = (
            "030644e72e131a029b85045b68181585d97816a916871ca8d3c208c16d87cfd3".to_owned(),
            "0x15ed738c0e0a7c92e7845f96b2ae9c0a68a6a449e3538fc7ff3ebf7a5a18a2c4".to_owned()
        );
        let pk_from_hex = PublicKey::from_hex_string(hex_coords);
        assert!(!pk_from_hex.is_ok())
    }
}
