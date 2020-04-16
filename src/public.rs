#![allow(non_snake_case)]
extern crate rand;

use bn::{Fr, Group, G1, AffineG1};

use bincode::rustc_serialize::encode;
use bincode::SizeLimit::Infinite;
use rand::thread_rng;
use sha2::{Digest, Sha512};

use crate::ciphertext::*;

/// The `PublicKey` struct represents an ElGamal public key.
#[derive(Copy, Clone)]
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
        // todo: version of rand crate is pretty old for this to work.
        let random: Fr = Fr::random(rng);

        let random_generator = G1::one() * random;
        let encrypted_plaintext = *message + self.0 * random;
        // random.clear(); todo:no clearing with Fr
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
    pub fn get_point_string(&self) -> (String, String) {
        get_point_as_str(self.0)
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
    ///    let proof = sk.prove_correct_decryption_no_Merlin(&ciphertext, &decryption);
    ///
    ///    assert!(pk.verify_correct_decryption_no_Merlin(proof, ciphertext, decryption));
    /// # }
    /// ```
    pub fn verify_correct_decryption_no_Merlin(
        self,
        proof: ((G1, G1), Fr),
        ciphertext: Ciphertext,
        message: G1,
    ) -> bool {
        let ((announcement_base_G, announcement_base_ctxtp0), response) = proof;
        let hash = Sha512::new()
            .chain(encode(&message, Infinite).unwrap())
            .chain(encode(&ciphertext.points.0, Infinite).unwrap())
            .chain(encode(&ciphertext.points.1, Infinite).unwrap())
            .chain(encode(&announcement_base_G, Infinite).unwrap())
            .chain(encode(&announcement_base_ctxtp0, Infinite).unwrap())
            .chain(encode(&G1::one(), Infinite).unwrap())
            .chain(encode(&self.get_point(), Infinite).unwrap());

        let mut output = [0u8; 64];
        output.copy_from_slice(hash.result().as_slice());
        let challenge = Fr::interpret(&output);

        G1::one() * response == announcement_base_G + self.get_point() * challenge
            && ciphertext.points.0 * response
                == announcement_base_ctxtp0 + (ciphertext.points.1 - message) * challenge
    }
}

pub fn get_point_as_str(point: G1) -> (String, String) {
    let point = AffineG1::from_jacobian(point).unwrap();
    let coords_x = point.x().into_u256().0;
    let coords_y = point.y().into_u256().0;

    if coords_x[1] == 0 && coords_y[1] == 0 {
        return (
            coords_x[0].to_string(),
            coords_y[0].to_string()
        )
    }

    else if coords_x[1] == 0 {
        return (
            coords_x[0].to_string(),
            coords_y[0].to_string() + &coords_y[1].to_string()
        )
    }

    else if coords_y[1] == 0 {
        return (
            coords_x[0].to_string() + &coords_x[1].to_string(),
            coords_y[0].to_string()
        )
    }

    else {
        return (
            coords_x[0].to_string() + &coords_x[1].to_string(),
            coords_y[0].to_string() + &coords_y[1].to_string()
        )
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use rand::thread_rng;

    #[test]
    fn test_string_conversion() {
        let pk = PublicKey::from(G1::one());
        let pk_string = pk.get_point_string();
        assert_eq!(pk_string.0, "1");
        assert_eq!(pk_string.1, "2");
    }
}