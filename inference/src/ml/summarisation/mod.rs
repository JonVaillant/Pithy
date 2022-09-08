use std::error::Error;
use std::fs;
use std::path::Path;
use std::result::Result;
use onnxruntime;
use onnxruntime::GraphOptimizationLevel;
use onnxruntime::LoggingLevel;
use onnxruntime::environment::Environment;
use onnxruntime::ndarray;
use onnxruntime::ndarray::ArrayBase;
use onnxruntime::ndarray::Dim;
use onnxruntime::ndarray::OwnedRepr;
use onnxruntime::session::Session;
use onnxruntime::tensor::OrtOwnedTensor;
use tokenizers::Tokenizer;

use crate::ml::utils::tokenizer_utils::TokenizerUtils;
use crate::ml::{utils::model_utils::Config};

#[cfg_attr(feature = "summarisation_system_alloc", global_allocator)]
#[cfg(feature = "summarisation_system_alloc")]
static ALLOCATOR: std::alloc::System = std::alloc::System;

pub fn main() -> Result<(), Box<dyn Error>> {
    let model_path_input = std::env::args()
        .skip(1)
        .next()
        .expect("Must provide a path to the model's directory");

    let model_path = Path::new(&model_path_input);

    let path_text = std::env::args()
        .skip(2)
        .next()
        .expect("Must provide a .txt file as the second arg");

    let text_content: String = fs::read_to_string(path_text)?.parse()?;

    let tokenizer = Tokenizer::from_path(model_path);

    // Scary "Ġ" character is normal: https://github.com/huggingface/transformers/issues/4786
    let encoding = tokenizer?.encode(text_content, true).unwrap();

    println!("{:?}", encoding.get_tokens());

    let ids = encoding
        .get_ids()
        .iter()
        .map(|x| *x as i64)
        .collect::<Vec<i64>>();
    let id_shape = (1, ids.len());
    let id_array: ArrayBase<OwnedRepr<i64>, Dim<[usize; 2]>> = ndarray::Array::from_shape_vec(id_shape, ids.clone()).unwrap();

    let attention_mask = encoding
        .get_attention_mask()
        .iter()
        .map(|x| *x as i64)
        .collect::<Vec<i64>>();

    let mask_shape = (1, attention_mask.len());
    let mask_array = ndarray::Array::from_shape_vec(mask_shape, attention_mask.clone()).unwrap();

    let environment = Environment::builder()
        .with_name("app")
        .with_log_level(LoggingLevel::Verbose)
        .build()?;

    // note some models silently fail (for example model.onnx silently failed but encoder_model.onnx succeeded)
    // probably because not all their inputs were satisfied
    let mut session_encoder = environment
        .new_session_builder()?
        .with_optimization_level(GraphOptimizationLevel::All)?
        // .with_number_threads(4)?
        .with_model_from_file(model_path.join("encoder_model.onnx"))?;

    // let config = Config::from_file(&model_path.join("config.json"))?;

    print_inputs_outputs(&session_encoder);

    // See example on line 91: https://github.com/JonVaillant/onnxruntime-rs/blob/master/onnxruntime/src/lib.rs
    let array_0: ArrayBase<OwnedRepr<i64>, Dim<[usize; 2]>> = id_array;
    let array_1: ArrayBase<OwnedRepr<i64>, Dim<[usize; 2]>> = mask_array;
    let input_tensor_values = vec![array_0, array_1];

    let outputs_encoder: Vec<OrtOwnedTensor<f32, _>> = session_encoder.run(input_tensor_values)?;
   

    println!("after encoder");


    let mut session_decoder = environment
        .new_session_builder()?
        .with_optimization_level(GraphOptimizationLevel::All)?
        // .with_number_threads(4)?
        .with_model_from_file(model_path.join("decoder_model.onnx"))?;

    // These ids are the encoder's sole output "last_hidden_state"
    let encoder_ids = outputs_encoder[0].as_slice().unwrap().into_iter().map(|x| *x as i64).collect::<Vec<i64>>(); // Rust actually converts values when you use `as` (not just type)
    let encoder_id_shape = (1, encoder_ids.len());
    let encoder_id_array: ArrayBase<OwnedRepr<i64>, Dim<[usize; 2]>> = ndarray::Array::from_shape_vec(encoder_id_shape, encoder_ids.clone()).unwrap();
    // ^ Interface wants them IDs to be i64, but the model wants them as float32

    let array_0_decoder: ArrayBase<OwnedRepr<i64>, Dim<[usize; 2]>> = ndarray::Array::from_shape_vec(id_shape, ids.clone()).unwrap(); // input_ids int64
    let array_1_decoder: ArrayBase<OwnedRepr<i64>, Dim<[usize; 2]>> = encoder_id_array; // encoder_hidden_states float32
    let array_2_decoder: ArrayBase<OwnedRepr<i64>, Dim<[usize; 2]>> = ndarray::Array::from_shape_vec(mask_shape, attention_mask.clone()).unwrap(); // encoder_attention_mask int64

    let input_tensor_values_decoder = vec![array_0_decoder, array_1_decoder, array_2_decoder];

    println!("Before decoder");

    let outputs_decoder: Vec<OrtOwnedTensor<f32, _>> = session_decoder.run(input_tensor_values_decoder)?;
        
    
    println!("after decoder");

    // assert_eq!(outputs[0].shape(), output0_shape.as_slice());
    // for i in 0..5 {
    //     println!("Score for class [{}] =  {}", i, outputs[0][[0, i, 0, 0]]);
    // }

    // # seem to use huggingface tokenizers lib with their own ONNX model consumed into onnxruntime
    // https://github.com/HIT-SCIR/libltp/blob/56689f6be39aa30350ea5755a804df19e461222a/ltp-rs/src/interface.rs
    // https://crates.io/crates/tokenizers

    
    // encoding.token_to_word(token)


    Ok(())
}

/** 
    Prints the model outputs for a given Session.

    ## Example output from printing a model

    Inputs:
    0:
        name = input_ids
        type = Int64
        dimensions = [None, None]
    1:
        name = attention_mask
        type = Int64
        dimensions = [None, None]
    2:
        name = decoder_input_ids
        type = Int64
        dimensions = [None, None]
    3:
        name = decoder_attention_mask
        type = Int64
        dimensions = [None, None]
    Outputs:
    0:
        name = logits
        type = Float
        dimensions = [None, None, Some(50264)]
    1:
        name = onnx::MatMul_2341
        type = Float
        dimensions = [None, None, Some(1024)]
*/
fn print_inputs_outputs (session: &Session) {
    println!("Inputs:");
    for (index, input) in session.inputs.iter().enumerate() {
        println!(
            "  {}:\n    name = {}\n    type = {:?}\n    dimensions = {:?}",
            index, input.name, input.input_type, input.dimensions
        )
    }

    println!("Outputs:");
    for (index, output) in session.outputs.iter().enumerate() {
        println!(
            "  {}:\n    name = {}\n    type = {:?}\n    dimensions = {:?}",
            index, output.name, output.output_type, output.dimensions
        );
    }
}
