# Ledger: goldilocks

Noir's `execution_success` corpus and local fixtures at the pinned compiler revision. Regenerate with `make ledger`; CI checks for changes. `source` and successful `projection` cells show the first 16 hash digits; projection failures show their cause. `oracle` compares the return with `Prover.toml`: exactly under bn254, ignoring `Field` values under goldilocks because the corpus records bn254 values.

| provenance | value |
| --- | --- |
| format | 3 |
| projection | 2 |
| field | goldilocks |
| modulus | 18446744069414584321 |
| noir_rev | 7db2450226e64bed450e65d55ec4f833e7cc498b |
| corpus_hash | b7b1c07fad362d06e8a05381cc4cb5683d87bb21503a937a2f8dd467c81f6a36 |
| programs | 509 |
| toolchain | rustc 1.89.0 (29483883e 2025-08-04) |
| features | goldilocks |

## Corpus

| program | source | load | compile | interpret | oracle | return | projection |
| --- | --- | --- | --- | --- | --- | --- | --- |
| a_1327_concrete_in_generic | 893eb4f3ddb1821e | ok | ok | ok | ok | 1 | 074ccc29677b37af |
| a_1_mul | e9734170d2c5ac25 | ok | ok | ok | n/a: no recorded return | () | 04663524a12f244e |
| a_2_div | 0a74ef799350e1e2 | ok | ok | ok | n/a: no recorded return | () | 5d1f5551771b7e17 |
| a_3_add | e7449657db4d7fb2 | ok | ok | ok | n/a: no recorded return | () | fa7d3c11a08515e0 |
| a_4_sub | d8ec1a4a914399c5 | ok | FAIL DependencyCompileGap: program reaches dependency code from files that failed elaboration under the chos…#d581b3ff | n/a: not compiled | n/a: not interpreted |  |  |
| a_5_over | 48e266078d70c2db | ok | FAIL DependencyCompileGap: program reaches dependency code from files that failed elaboration under the chos…#d581b3ff | n/a: not compiled | n/a: not interpreted |  |  |
| a_6 | 9f8a1e538230afa5 | ok | FAIL DependencyCompileGap: program reaches dependency code from files that failed elaboration under the chos…#81dd487c | n/a: not compiled | n/a: not interpreted |  |  |
| a_6_array | 5f475c7bbec85671 | ok | FAIL DependencyCompileGap: program reaches dependency code from files that failed elaboration under the chos…#d581b3ff | n/a: not compiled | n/a: not interpreted |  |  |
| a_7 | e4747d60088cd729 | ok | ok | FAIL Unsupported(intrinsic): intrinsic 'blake2s' | n/a: not interpreted |  | c9fea35bee6f2d59 |
| a_7_function | 4d5139b26c59abf7 | ok | FAIL DependencyCompileGap: program reaches dependency code from files that failed elaboration under the chos…#90ec0f4a | n/a: not compiled | n/a: not interpreted |  |  |
| aes128_encrypt | a37791cd97027465 | ok | ok | FAIL Unsupported(intrinsic): intrinsic 'aes128_encrypt' | n/a: not interpreted |  | b6651b9ecfce4e2b |
| arithmetic_binary_operations | 1509de7f78f245a8 | ok | ok | ok | ok | 10 | 7db796dd0dd88897 |
| array_dedup_regression | 601d66564e83a99e | ok | ok | FAIL Unsupported(oracle call): oracle call 'print' | n/a: not interpreted |  | 9f507d7af1505dba |
| array_dynamic | 66d1150db5838a76 | ok | ok | ok | n/a: no recorded return | () | 729dccfb4dbd4c4b |
| array_dynamic_blackbox_input | f74bf227f79a34f7 | ok | FAIL DependencyCompileGap: program reaches dependency code from files that failed elaboration under the chos…#81dd487c | n/a: not compiled | n/a: not interpreted |  |  |
| array_dynamic_main_output | 2f16cf44ffeb69b7 | ok | ok | ok | ok | [0, 1, 2, 3, 4, 0, 6, 7, 8, 9] | d6c728fd3867bb42 |
| array_dynamic_nested_blackbox_input | 5f5026a60d20d92a | ok | FAIL DependencyCompileGap: program reaches dependency code from files that failed elaboration under the chos…#81dd487c | n/a: not compiled | n/a: not interpreted |  |  |
| array_eq | f1b2d43a3ed965ae | ok | ok | ok | n/a: no recorded return | () | f11189ed02f3e12f |
| array_if_cond_simple | 9eb2a9926232f979 | ok | ok | ok | n/a: no recorded return | () | 93912af94baa351b |
| array_len | 564f4e6e20d3b920 | ok | FAIL DependencyCompileGap: program reaches dependency code from files that failed elaboration under the chos…#90ec0f4a | n/a: not compiled | n/a: not interpreted |  |  |
| array_of_references_in_loop | 1684252cfce84765 | ok | ok | ok | ok | true | 64b4a790ad029f0d |
| array_oob_regression_7965 | d10bb8dfe6af3c49 | ok | ok | ok | ok | [] | 31a0e62bfe95a9a7 |
| array_oob_regression_7975 | 5760ba2ed1e73137 | ok | ok | ok | ok | [[false]] | c173e4aba1f57eea |
| array_rc_regression_7842 | c7ce901347cbdaf3 | ok | ok | ok | n/a: no Prover.toml | () | cd91768e4fb9138a |
| array_set_not_deduplicated | a5a5e474fc013399 | ok | ok | FAIL Unsupported(oracle call): oracle call 'print' | n/a: not interpreted |  | f55f5aa172727958 |
| array_set_zero_length_element_brillig_input | 77e15600a88b04eb | ok | ok | ok | ok | [[]] | 8550862c9e77d706 |
| array_sort | 5a02e174f0e4d7b3 | ok | ok | ok | n/a: no recorded return | () | 47ac33696e236cf2 |
| array_to_vector | 532ae0d1499e08b4 | ok | ok | ok | n/a: no recorded return | () | a165b0ac1e00dc97 |
| array_to_vector_constant_length | cccd8ea600165fd1 | ok | ok | ok | n/a: no recorded return | () | 8e15a36714fe7c7c |
| array_with_refs_from_param | f12d3677e048a517 | ok | ok | ok | ok | true | 985809503eef948f |
| array_with_refs_return | 13efc775b9f0ade9 | ok | ok | ok | ok | [2i8, 3i8] | fd3337003ad62ee2 |
| as_str_unchecked_with_broken_bytes | 48cdfb6cb1732bae | ok | ok | FAIL Unsupported(array_as_str_unchecked on non-UTF-8 bytes): array_as_str_unchecked on non-UTF-8 bytes: inva…#ed4de2c2 | n/a: not interpreted |  | da8955dcf23ab98d |
| as_witness | a3dac9e7da07979b | ok | ok | ok | ok | 42 | f05900694f52b1da |
| assert | 449f8f087421c4b2 | ok | ok | ok | n/a: no recorded return | () | d7265771f144fcc9 |
| assert_statement | 9f0a79b4cb510e93 | ok | ok | ok | n/a: no recorded return | () | 4f43e2a95c8f22a9 |
| assign_ex | 9d40b3ebb05f3338 | ok | ok | ok | n/a: no recorded return | () | fd491c96c3700490 |
| associated_constant_as_array_length | 63f01b284d81ccb3 | ok | ok | ok | n/a: no recorded return | () | bcde4e9d7c3b2ef5 |
| bench_2_to_17 | 69f819a242995b28 | ok | FAIL DependencyCompileGap: program reaches dependency code from files that failed elaboration under the chos…#5da1f8a9 | n/a: not compiled | n/a: not interpreted |  |  |
| binary_operator_overloading | 2a09d80fe7629d23 | ok | ok | ok | n/a: no recorded return | () | 48d4af77f98292e8 |
| bit_and | 247b949084e79565 | ok | ok | ok | n/a: no recorded return | () | c9f772790828f964 |
| bit_not | 70be2df290a65516 | ok | ok | ok | n/a: no recorded return | () | eabdb139abbf233f |
| bit_shifts_comptime | a432f7a1aacc48f3 | ok | ok | ok | n/a: no recorded return | () | a6690e8dff9d35dd |
| bit_shifts_runtime | 532ce4239afe7ef4 | ok | ok | FAIL Unsupported(signed 64-bit ABI input is not representable in this field): signed 64-bit ABI input is not…#2a723acd | n/a: not interpreted |  | fdde95a95823e34e |
| bit_shifts_u128 | 48b3706b32d8938c | ok | ok | FAIL InputError: failed to parse Prover.toml: The value passed for parameter `x` is invalid: Value 340282366…#1e64363a | n/a: not interpreted |  | e09642af9e597847 |
| blake3 | 41aef05e322140ed | ok | FAIL DependencyCompileGap: program reaches dependency code from files that failed elaboration under the chos…#81dd487c | n/a: not compiled | n/a: not interpreted |  |  |
| bool_not | 0ae7e796313308c4 | ok | ok | ok | n/a: no recorded return | () | 99353271d9a982b9 |
| bool_or | 40a1c684ee5449b8 | ok | ok | ok | n/a: no recorded return | () | 1e6a9f21aa2ed35e |
| bounded_vec_extend_from_bounded_vec | 8ea648944ef5792e | ok | ok | ok | ok | [1u8, 2u8, 3u8, 4u8, 5u8, 6u8, 7u8, 8u8, 9u8, 10u8, 11u8, 12u8, 13u8, 14u8, 15u8, 16u8, 17u8, 18u8, 19u8, 20…#d59d146b | c8c77f69447d3ef0 |
| bounded_vec_extend_pattern | 962c2bc9ee4d31f3 | ok | ok | ok | ok | [1u8, 2u8, 3u8, 4u8, 5u8, 6u8, 7u8, 8u8, 9u8, 10u8, 11u8, 12u8, 13u8, 14u8, 15u8, 16u8, 17u8, 18u8, 19u8, 20…#d59d146b | b3e58bb32fc881e5 |
| break_and_continue | 5db551505b47df5b | ok | ok | ok | n/a: no Prover.toml | () | 6f0102fd22d485fe |
| brillig_acir_as_brillig | 84691e5a16dad117 | ok | ok | ok | n/a: no recorded return | () | db763eaf3500b64d |
| brillig_array_ifelse | 85eea7bb848536f0 | ok | ok | ok | ok | (true, [false], [true]) | a0cef3263c9a50d2 |
| brillig_array_input_indirectly_mutated | 6987df8332b230b0 | ok | ok | ok | ok | [true] | 881bba628c883d8f |
| brillig_arrays | ee2986bc5f1110c6 | ok | ok | ok | n/a: no recorded return | () | 0c78300aee48ebee |
| brillig_blake2s | d7f5cf6a4eb24440 | ok | ok | FAIL Unsupported(intrinsic): intrinsic 'blake2s' | n/a: not interpreted |  | 44f0c3586784e52f |
| brillig_block_mutable_reference_index_regression | 8a78bb170fb8f23e | ok | ok | ok | ok | [1u32, 2u32, 3u32] | 5830c9f42be3fc8c |
| brillig_block_mutable_reference_inner_ref_regression | af16f07a1dd43ff5 | ok | ok | ok | ok | [1u32, 2u32, 3u32] | fb9ad7b818536dc1 |
| brillig_block_mutable_reference_let_chain_long_regression | 04b9faaefe745a88 | ok | ok | ok | ok | [1u32, 2u32, 3u32] | 1b3212d440555a7a |
| brillig_block_mutable_reference_let_chain_regression | dab05582436e393b | ok | ok | ok | ok | [1u32, 2u32, 3u32] | 1e05cae9c9c653ae |
| brillig_block_mutable_reference_regression | 2a92bb84092695ec | ok | ok | ok | ok | [1u32, 2u32, 3u32] | 611e533b3fad64e9 |
| brillig_block_parameter_liveness | 86b8d6042d00f9da | ok | ok | ok | ok | (((1u64, 0u64, 5u64, 0u64, 0u64, 9u64, 0u64, 0u64, 0u64, 0u64), (0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0…#c587f77d | 618862c9305b51db |
| brillig_calls | cbbe272a8ab25d57 | ok | ok | ok | n/a: no recorded return | () | c6d957c85d7564f2 |
| brillig_calls_array | 6a3aaa63991c44ed | ok | ok | ok | n/a: no recorded return | () | 2473ed0dbc212796 |
| brillig_calls_conditionals | 261519d642ae7411 | ok | ok | ok | n/a: no recorded return | () | 336f9e3e77b19eca |
| brillig_conditional | fb0689373f20e067 | ok | ok | ok | n/a: no recorded return | () | dec30bcdf68f323e |
| brillig_constant_reference_regression | 82f26387dc3c6801 | ok | ok | ok | n/a: no recorded return | () | 57501fe9a04e92f2 |
| brillig_cow | 07f20ad8b43389d7 | ok | ok | ok | n/a: no recorded return | () | 4177cc66001400f1 |
| brillig_cow_assign | b624c93ef29da313 | ok | ok | ok | n/a: no recorded return | () | 6b9e4e23f620404a |
| brillig_cow_regression | 208b9b1c1ee7f891 | ok | FAIL Panic: internal error: entered unreachable code: Encountered Error node during monomorphization | n/a: not compiled | n/a: not interpreted |  |  |
| brillig_entry_points_regression_8069 | 41df366b33a6991c | ok | ok | ok | ok | false | 1fcd6aeddc3956ec |
| brillig_fns_as_values | 5dabd31b95e0bd77 | ok | ok | ok | n/a: no recorded return | () | f483b1a5c4460867 |
| brillig_identity_function | dba2f62bb6f6bd51 | ok | ok | ok | n/a: no recorded return | () | e07915a207aaa74f |
| brillig_if_mutable_reference_regression | bf44d76b9c61fa6f | ok | ok | ok | n/a: no Prover.toml | () | 4b4cf68d14571f19 |
| brillig_large_array | 4232c6ea0fe7ce90 | ok | ok | ok | n/a: no recorded return | () | 7fa66f342e1b9294 |
| brillig_large_nested_array | c9cda5d02859095a | ok | ok | ok | n/a: no recorded return | () | 7f7129a47afd46e2 |
| brillig_loop_bound_upper_below_lower | 418f6b9c77b44584 | ok | FAIL DependencyCompileGap: program reaches dependency code from files that failed elaboration under the chos…#d581b3ff | n/a: not compiled | n/a: not interpreted |  |  |
| brillig_loop_size_regression | 758822209e87719e | ok | ok | ok | ok | 2 | 46e19aeb37814cf7 |
| brillig_mutable_reference_lsf_bug | bfa41af682226f54 | ok | ok | ok | ok | true | e15c4b133f926144 |
| brillig_nested_arrays | b2ccc25e7a25fd16 | ok | ok | ok | n/a: no recorded return | () | 69da37360a941273 |
| brillig_not | 222815631658d0f5 | ok | ok | ok | n/a: no recorded return | () | 1811190a47be4966 |
| brillig_pedersen | 6dde5a6cc984d55f | ok | FAIL Panic: internal error: entered unreachable code: Encountered Error node during monomorphization | n/a: not compiled | n/a: not interpreted |  |  |
| brillig_rc_regression_6123 | b22612f281657fb5 | ok | ok | FAIL Unsupported(dereference of a non-reference value): dereference of a non-reference value (Tuple([RefCell…#10d288c2 | n/a: not interpreted |  | 15af421d32634ecb |
| brillig_recursion | f04cac7814099a7a | ok | ok | ok | n/a: no recorded return | () | 835d7508a506b8d8 |
| brillig_recursive_main | eefcabf5a76588c9 | ok | ok | ok | ok | true | 2eaadd03c6d52dfc |
| brillig_recursive_main_indirect | e574d4660021b7e9 | ok | ok | ok | ok | true | f9f9526be6cb041b |
| brillig_uninitialized_arrays | 60b07ec6a8d082c7 | ok | ok | ok | ok | 1 | dc29e48c9789f2e3 |
| cast_bool | a09ddc2beef23eb1 | ok | ok | ok | n/a: no recorded return | () | 25d5dda7d134619c |
| cast_regression_7776 | b79ca4801f112104 | ok | ok | ok | FAIL OracleMismatch: tuple[2]: integer differs: 18446744069414584320 (s=false,64b) vs 4891460686036598784 (s=false,64b) | (0i8, 0u8, 18446744069414584320u64) | 5f17c4abedd73434 |
| cast_signed_to_u1 | e2a895d4d511f8d4 | ok | ok | ok | n/a: no recorded return | () | d77bb097a879c7fc |
| cast_to_u128 | 3e4649c077445e28 | ok | ok | ok | n/a: no recorded return | () | c5904ee2b99bb815 |
| chained_associated_type_in_signature | fc56529e89227213 | ok | ok | ok | n/a: no recorded return | () | a780644068d944be |
| clone_index_field_dereference | 9475e3b358994df8 | ok | ok | ok | n/a: no recorded return | () | 806c2a297b918821 |
| clone_index_object_dereference_1 | 3ba58ea770dfb97f | ok | ok | ok | n/a: no recorded return | () | c09da196a5a64b8a |
| clone_index_object_dereference_2 | ec135f9cb0dd1d09 | ok | ok | ok | n/a: no recorded return | () | ea368c426ca4ccb1 |
| closures_mut_ref | 8d9e95bd9f9dfb77 | ok | ok | ok | n/a: no recorded return | () | 85f91472b06a6416 |
| comptime_closure_bindings_1 | a5bca749dc2d8446 | ok | ok | ok | n/a: no Prover.toml | () | f31756733317f3c5 |
| comptime_closure_bindings_2 | d50f56e49a0677cc | ok | ok | ok | n/a: no Prover.toml | () | 97b8039b1ca58a4a |
| comptime_function_definition_attribute_arg | 699201b569740777 | ok | ok | ok | n/a: no recorded return | () | 891d6b603247cedf |
| comptime_generics_binding | 249af2f582c1dece | ok | ok | ok | n/a: no recorded return | () | a4bc62e55f7c433c |
| comptime_named_attribute_args | 657d1481cafa9112 | ok | ok | ok | n/a: no recorded return | () | 062cb6882c187048 |
| comptime_println | d45807f1ff750acf | ok | ok | FAIL Unsupported(oracle call): oracle call 'print' | n/a: not interpreted |  | a7bdf26ac90bf9e7 |
| comptime_println_fmtstr_with_quoted | 702e6999ebfd179e | ok | ok | ok | n/a: no Prover.toml | () | 68a6ff7efe99b607 |
| comptime_quoted_hash | 222cf8713b07b514 | ok | ok | ok | n/a: no Prover.toml | () | 72181413ce2f8eda |
| comptime_resolve_associated_constant_scope | b71ba4244b709299 | ok | ok | ok | n/a: no recorded return | () | 674ad36247bb364a |
| comptime_trait_constraint_hash_and_eq | 343c1eb3881c6010 | ok | ok | ok | n/a: no Prover.toml | () | 72181413ce2f8eda |
| comptime_variable_at_runtime | 99a4364fbc775b11 | ok | ok | FAIL Unsupported(oracle call): oracle call 'print' | n/a: not interpreted |  | f013fa5c0c45c5b1 |
| conditional_1 | 4b97750f3eb6bff5 | ok | FAIL DependencyCompileGap: program reaches dependency code from files that failed elaboration under the chos…#81dd487c | n/a: not compiled | n/a: not interpreted |  |  |
| conditional_2 | 28e33182770035aa | ok | ok | ok | n/a: no recorded return | () | c057eb1589dcae39 |
| conditional_black_box_function_pointer_call | cfcdc0f9b3c36545 | ok | ok | FAIL Unsupported(intrinsic): intrinsic 'blake2s' | n/a: not interpreted |  | d3adecf2b52cc34e |
| conditional_regression_421 | 273dc3d7c4a5a2c8 | ok | ok | ok | n/a: no recorded return | () | f19863acce4fa62b |
| conditional_regression_547 | d0e88a01db6c9d1a | ok | ok | ok | ok | 1 | 3c5088d58ac6ec6b |
| conditional_regression_661 | 554e636e9a3c39e6 | ok | ok | ok | n/a: no recorded return | () | 280accddb5c09a17 |
| conditional_regression_short_circuit | 689f4c2c5ec713ba | ok | FAIL DependencyCompileGap: program reaches dependency code from files that failed elaboration under the chos…#81dd487c | n/a: not compiled | n/a: not interpreted |  |  |
| conditional_regression_underflow | df054826ea3b2a99 | ok | ok | ok | n/a: no recorded return | () | ae8bc687c5ddeef6 |
| conditional_vector_insert_at_end_of_vector | b89fd8336380768a | ok | ok | ok | ok | 4u32 | 86f9ddad14c67b42 |
| constant_folding_mutated_returned_array_bug | 4f0758c7847b6df8 | ok | ok | ok | ok | [true] | 006875784a7bb265 |
| custom_entry | 5830f34bc9f62da0 | ok | ok | ok | n/a: no recorded return | () | 33b4e722aee996ad |
| databus | 8ebce5f2f7732150 | ok | ok | ok | ok | 9u32 | 1075a4ab729253a7 |
| databus_composite_calldata | 862ca681e0985c4a | ok | ok | ok | ok | 2u32 | c0bfa339c3a683e2 |
| databus_two_calldata | ea78f8efcabe3b12 | ok | ok | ok | ok | [1u32, 5u32, 9u32, 7u32] | 88e9b639b3301074 |
| databus_two_calldata_simple | aeaa24f0def1905f | ok | ok | ok | ok | 11u32 | d30b5ae290bd1531 |
| debug_logs | 26e6fd6a79083aa5 | ok | ok | FAIL Unsupported(oracle call): oracle call 'print' | n/a: not interpreted |  | dd601be595f757d4 |
| debug_name_no_conflict | 1f265b8f383c7d94 | ok | ok | ok | n/a: no recorded return | () | cb38f359f171c86b |
| defunctionalize_mut_ref_to_immut_ref_regression | 33d3c1fe5686e987 | ok | ok | ok | ok | 5 | 07b669dfca59c2e0 |
| dereference_assignment | 586b17bd30e902bc | ok | ok | ok | n/a: no recorded return | () | f2979890f588b8a9 |
| derive | 598e1538b9c24ce4 | ok | FAIL CompileError: No method named 'do_nothing' found for type 'MyStruct' \| Could not resolve 'default' in …#11e3efba | n/a: not compiled | n/a: not interpreted |  |  |
| diamond_deps_0 | 91f88698490b2ae6 | ok | ok | ok | ok | 7 | df89fa095dfa1833 |
| division_by_max | f6dae6db80cd3504 | ok | ok | ok | ok | 36u8 | 34f007d01030c2b6 |
| do_not_capture_comptime_locals | 003fed56c4e1f8ee | ok | ok | FAIL Unsupported(oracle call): oracle call 'print' | n/a: not interpreted |  | 0a77eabdd3a47614 |
| dont_deduplicate_call | b0ab11bd8904aa73 | ok | ok | ok | n/a: no recorded return | () | 0fee6b16835a8a5e |
| double_neg_cond_bool_input | e97e4f018dbbcea6 | ok | ok | ok | ok | 1 | c575ac847028bd71 |
| double_neg_cond_global_var | 2846e4ebabe15fe0 | ok | ok | ok | ok | 1 | 57565def9c7451ea |
| dual_constrained_lambdas | 1d156883ff8c3b56 | ok | ok | ok | n/a: no recorded return | () | 20ffcd3ecda4acfc |
| ecdsa_secp256k1 | 5ab1bc625863ce9d | ok | ok | FAIL Unsupported(intrinsic): intrinsic 'ecdsa_secp256k1' | n/a: not interpreted |  | 0af2bfdd931b95aa |
| ecdsa_secp256k1_invalid_inputs | cccee2a2571c98d1 | ok | ok | FAIL Unsupported(intrinsic): intrinsic 'ecdsa_secp256k1' | n/a: not interpreted |  | 9dfcc04039fdb831 |
| ecdsa_secp256k1_invalid_pub_key_in_inactive_branch | c622b63c16a25386 | ok | ok | ok | n/a: no recorded return | () | 36ba56561493712b |
| ecdsa_secp256k1_msg_equals_order | 34c48a440a34fc81 | ok | ok | FAIL Unsupported(intrinsic): intrinsic 'ecdsa_secp256k1' | n/a: not interpreted |  | f160135d71a2c95f |
| ecdsa_secp256r1 | 969b72d32dd2c088 | ok | ok | FAIL Unsupported(intrinsic): intrinsic 'ecdsa_secp256r1' | n/a: not interpreted |  | 9e85af328a58c991 |
| ecdsa_secp256r1_3x | d51f99c52a28a9bc | ok | ok | FAIL Unsupported(intrinsic): intrinsic 'ecdsa_secp256r1' | n/a: not interpreted |  | f2e479a6cbfdcdf9 |
| ecdsa_secp256r1_high_s | baad4aec7fbb6bea | ok | ok | FAIL Unsupported(intrinsic): intrinsic 'ecdsa_secp256r1' | n/a: not interpreted |  | 036be7bbe6c8b6bf |
| ecdsa_secp256r1_invalid_inputs | a8135d5551ed86df | ok | ok | FAIL Unsupported(intrinsic): intrinsic 'ecdsa_secp256r1' | n/a: not interpreted |  | 6fa9b5a9ffc30ecc |
| ecdsa_secp256r1_invalid_pub_key_in_inactive_branch | f8eaaa1cb4f24b1f | ok | ok | ok | n/a: no recorded return | () | 67ef3e827c56ccc4 |
| embedded_curve_ops | f62b3e73968abfa2 | ok | FAIL CompileError: The value `14538976827940032377410132001999431907803857905256930177378457447038053842343`…#71c4f794 | n/a: not compiled | n/a: not interpreted |  |  |
| empty_strings_in_composite_arrays | 1f98227246f79081 | ok | ok | FAIL Unsupported(oracle call): oracle call 'print' | n/a: not interpreted |  | e7f1386a684b3eda |
| encrypted_log_regression | 6691440864c1f6d5 | ok | ok | ok | ok | [1u8, 2u8, 3u8, 1u8, 2u8, 0u8, 9u8, 8u8, 7u8, 6u8, 5u8, 4u8, 3u8, 2u8, 1u8] | fbb7d61964c7e4f5 |
| field_attribute | daf2370c53001245 | ok | FAIL CompileError: cannot find `foo` in this scope | n/a: not compiled | n/a: not interpreted |  |  |
| fmtstr_with_global | e3aff25464e26103 | ok | ok | FAIL Unsupported(format string interpolating a field): format string interpolating a field | n/a: not interpreted |  | ce3b9d892a5ceba2 |
| fold_2_to_17 | 79843e2ce64bedd8 | ok | FAIL DependencyCompileGap: program reaches dependency code from files that failed elaboration under the chos…#5da1f8a9 | n/a: not compiled | n/a: not interpreted |  |  |
| fold_after_inlined_calls | ce7431500c3e7c03 | ok | ok | ok | n/a: no recorded return | () | 1e2467d29326de2e |
| fold_basic | 379f68ca2ddb6bb2 | ok | ok | ok | n/a: no recorded return | () | 7c641bf454d040f0 |
| fold_basic_nested_call | 793d4a5a148118a8 | ok | ok | ok | n/a: no recorded return | () | 9b51c7e208b2a505 |
| fold_call_witness_condition | a5fc8f3b6f172286 | ok | ok | ok | ok | [0, 0] | 7b09e1c5f1edf990 |
| fold_complex_outputs | 8da110d803215d96 | ok | ok | ok | n/a: no recorded return | () | 8eba555dd6fda517 |
| fold_distinct_return | 0aae6efb7caf66db | ok | ok | ok | n/a: no recorded return | () | 265ee1b233562e7e |
| fold_fibonacci | 82da31612085d5d4 | ok | ok | ok | n/a: no recorded return | () | f9fb9c8dc7c89884 |
| fold_numeric_generic_poseidon | f66d85c0721e11ba | ok | FAIL DependencyCompileGap: program reaches dependency code from files that failed elaboration under the chos…#5da1f8a9 | n/a: not compiled | n/a: not interpreted |  |  |
| for_loop_inclusive_empty_range | cc26f639116c4e3b | ok | ok | ok | n/a: no recorded return | () | 1afae202c83aaab9 |
| for_loop_inclusive_u8_max | cc6cd1e33be94ec8 | ok | ok | ok | n/a: no Prover.toml | () | 24b9a2fcbd0baa77 |
| for_loop_inclusive_with_break | 9347a4e459048595 | ok | ok | ok | n/a: no Prover.toml | () | cc58eef27d496e58 |
| function_ref | cdc0ca45ef02a7e0 | ok | ok | ok | ok | "bar" | bc8020f366b9337a |
| generics | c19310f70cc3776e | ok | ok | ok | n/a: no recorded return | () | 5abfd43688378863 |
| global_array_rc_regression_8259 | 1d660794d7dc1469 | ok | ok | ok | ok | [true, false, true] | 573f77a1105a0739 |
| global_consts | a0b8ec8a8b3bbb22 | ok | ok | ok | n/a: no recorded return | () | 5d1bb5b4e4ea3814 |
| global_nested_array_call_arg_regression | d7048ea5044ffb84 | ok | ok | ok | ok | false | 5000c38d40da9b00 |
| global_nested_array_regression_9270 | f67844fc909fa436 | ok | ok | ok | ok | [["DS"], ["EN"]] | 048993ae2b64e9a6 |
| global_var_entry_point_used_in_another_entry | 78f6b0c99be36b4a | ok | ok | ok | n/a: no recorded return | () | 3db2869d72ad7d8f |
| global_var_func_with_multiple_entry_points | 7613cfb3d8658f53 | ok | ok | ok | n/a: no recorded return | () | 231b4162c15923d0 |
| global_var_multiple_entry_points_nested | 6fcfee86210aecc3 | ok | ok | ok | n/a: no recorded return | () | 810118dcc73ff73f |
| global_var_regression_entry_points | 3d91f5fd43feadbf | ok | FAIL Panic: internal error: entered unreachable code: Encountered Error node during monomorphization | n/a: not compiled | n/a: not interpreted |  |  |
| global_var_regression_simple | ee86994546edbb37 | ok | FAIL Panic: internal error: entered unreachable code: Encountered Error node during monomorphization | n/a: not compiled | n/a: not interpreted |  |  |
| global_vector_rc_regression_8259 | fb35ff4d71eb66bf | ok | ok | FAIL Unsupported(oracle call): oracle call 'print' | n/a: not interpreted |  | 330e81d514c0cdb9 |
| higher_order_functions | 247e5a2626ff752c | ok | FAIL DependencyCompileGap: program reaches dependency code from files that failed elaboration under the chos…#4d924750 | n/a: not compiled | n/a: not interpreted |  |  |
| hint_black_box | 749cb75ceb98a3db | ok | ok | ok | n/a: no recorded return | () | 2ea7f612db17ad4b |
| if_else_chain | c29d073ac4c4ed49 | ok | ok | ok | n/a: no recorded return | () | 9a54b8357e2560d9 |
| immutable_ref_to_unconstrained | 3fa5fc57d440a970 | ok | ok | ok | n/a: no recorded return | () | 7f982632f91674d4 |
| import | dc23ab06e6b137ee | ok | FAIL Panic: internal error: entered unreachable code: Encountered Error node during monomorphization | n/a: not compiled | n/a: not interpreted |  |  |
| inactive_signed_bitshift | eedd33c80e0768b2 | ok | ok | ok | ok | 0i16 | c622487df6624f91 |
| inline_decompose_hint_brillig_call | c79204988d53e225 | ok | FAIL CompileError: The value `16215005577652802630133446360800746821800024031854490445241664420099146410003`…#d27510fd | n/a: not compiled | n/a: not interpreted |  |  |
| inline_never_basic | 5cc7e5118335c5b8 | ok | ok | ok | n/a: no recorded return | () | 578b6188e002a722 |
| integer_array_indexing | 8d012b2b50b8f2f2 | ok | ok | ok | ok | 8 | e8730c2dfc80eed0 |
| lambda_env_is_copied | 31ceb340d5dca4d9 | ok | ok | ok | n/a: no Prover.toml | () | cec34cddeb9e2be2 |
| lambda_from_array | cce0e975a6e3b97c | ok | ok | FAIL Unsupported(oracle call): oracle call 'print' | n/a: not interpreted |  | d29f249516d5c307 |
| lambda_from_dynamic_if | 474f4999a6cfd998 | ok | ok | FAIL Unsupported(oracle call): oracle call 'print' | n/a: not interpreted |  | 05fe3b05de11c4c5 |
| lambda_from_global_array | bf84fc77bfc77988 | ok | ok | ok | n/a: no recorded return | () | 34430bf8347a71d1 |
| lambda_from_global_tuple | 5d777d3fb609ef18 | ok | ok | ok | n/a: no recorded return | () | f8d428131dd9344b |
| lambda_taking_lambda_regression_8543 | a5d356237036f010 | ok | ok | ok | FAIL Unsupported(signed 64-bit ABI input is not representable in this field): signed 64-bit ABI input is not…#2a723acd | 0i64 | 6fa1f05b8272e885 |
| lambda_taking_lambda_with_variant | 36579be3ee799b13 | ok | ok | ok | FAIL Unsupported(signed 64-bit ABI input is not representable in this field): signed 64-bit ABI input is not…#2a723acd | 0i64 | bec5df2d4e1ee88c |
| large_nested_array_merge_loop | 9adbbb88dec7201a | ok | ok | ok | n/a: no recorded return | () | 3fff56d0476540f5 |
| large_nested_array_multi_field_merge | a703ae14f3f11910 | ok | ok | ok | n/a: no recorded return | () | 5007e64df59a8256 |
| large_nested_array_multi_field_merge_u64 | 03eeedc8cfa88bec | ok | ok | ok | n/a: no recorded return | () | 65145e8571ef6bb1 |
| last_uses_regression_8935 | 5eb34deceb5aaf20 | ok | ok | FAIL Unsupported(oracle call): oracle call 'print' | n/a: not interpreted |  | 462832894d3f6f05 |
| licm_bug_inverted_loop | 531ebdef7424acdc | ok | ok | FAIL Unsupported(oracle call): oracle call 'print' | n/a: not interpreted |  | 159e47f288f7a62f |
| local_module_does_not_conflict_with_debugger | faa4120025808482 | ok | ok | ok | n/a: no Prover.toml | () | 68a6ff7efe99b607 |
| loop | 3342453786888147 | ok | ok | ok | n/a: no recorded return | () | 78734a4436bd28e6 |
| loop_break_regression_8319 | fe152be941e12513 | ok | ok | ok | n/a: no recorded return | () | 34c1e43050d40090 |
| loop_buffer_rotation_aliasing_regression | 92f5e31f6e341f24 | ok | ok | ok | n/a: no recorded return | () | 15d0c90ce07bcff3 |
| loop_carried_aliases | f8f8e4efa3217700 | ok | ok | ok | ok | (3735928559, 3735928559) | 0379f7a80d387b06 |
| loop_invariant_nested_deep | 4f4a30b4280c9f35 | ok | ok | ok | n/a: no recorded return | () | 47f9418bbb774d74 |
| loop_invariant_regression | 1766c5d007ca20b3 | ok | ok | ok | n/a: no recorded return | () | 6ac9baeea48bd3b6 |
| loop_invariant_regression_8586 | 969e607e75d8d281 | ok | ok | ok | n/a: no recorded return | () | 6b0ca40942240d69 |
| loop_small_break | 651aabbb9f618c83 | ok | ok | ok | n/a: no recorded return | () | 63bee671ab958816 |
| main_bool_arg | 0b85a46bab54a6dc | ok | ok | ok | n/a: no recorded return | () | 2045ed412dc3c25f |
| main_return | 44ce62a297425cfc | ok | ok | ok | ok | 8 | ebca352062f7b8a5 |
| match_struct_pattern_field_order | 60be84b55da8410c | ok | ok | ok | n/a: no recorded return | () | 2a06a755c5e4e473 |
| merkle_insert | 961c7d9f5d6a2c71 | ok | FAIL Panic: internal error: entered unreachable code: Encountered Error node during monomorphization | n/a: not compiled | n/a: not interpreted |  |  |
| missing_closure_env | 23ba373a94d42130 | ok | ok | ok | n/a: no recorded return | () | ffcd375d98975ceb |
| modules | 89ad087cb489e3b4 | ok | ok | ok | n/a: no recorded return | () | 2c2c889a6b6c0752 |
| modules_more | dbd0439098ad4aca | ok | ok | ok | n/a: no recorded return | () | 46c17f0d3e604a91 |
| modulus | c25ee07586aeef06 | ok | ok | FAIL AssertionFailed | n/a: not interpreted |  | bfbe1ab1ac4eecca |
| multi_scalar_mul | b9f8cba029c9522c | ok | FAIL DependencyCompileGap: program reaches dependency code from files that failed elaboration under the chos…#b443ba55 | n/a: not compiled | n/a: not interpreted |  |  |
| mutable_and_immutable_reference_alias | 56a6ebf869cb9378 | ok | ok | ok | ok | 2 | 546f9d857e95f50b |
| mutate_array_copy | 0cd241093aeaa52e | ok | ok | ok | n/a: no Prover.toml | () | b7de7537ea7040d0 |
| negated_jmpif_condition | 19e039dfaa43cc51 | ok | ok | ok | n/a: no recorded return | () | 8a5e44d7f876f7e9 |
| negative_associated_constants | 09f8f73665e5a199 | ok | ok | FAIL Unsupported(oracle call): oracle call 'print' | n/a: not interpreted |  | bd0c691c1a3d85a6 |
| nested_array_call_arg_regression | 5ccc47c2c30d45e1 | ok | ok | ok | ok | false | 3ceab484f4ccae68 |
| nested_array_dynamic | 627486784a679765 | ok | ok | ok | n/a: no recorded return | () | b2918c627c8448b8 |
| nested_array_dynamic_simple | b344a8e6004ae38d | ok | ok | ok | n/a: no recorded return | () | 9b4d7fd8f1d1560b |
| nested_array_in_vector | 1462415647852ae1 | ok | ok | ok | n/a: no recorded return | () | eba6019a9201bf78 |
| nested_array_index_clone_regression | 03a1c91bd9393acf | ok | ok | FAIL Unsupported(oracle call): oracle call 'print' | n/a: not interpreted |  | 581a6986a15a5584 |
| nested_array_with_refs | b37f75b1e0b49497 | ok | ok | ok | ok | false | 1f6008510931b306 |
| nested_array_with_refs_from_param | 4417d8a133260832 | ok | ok | ok | ok | true | 9e4b29f0ecf5596c |
| nested_array_with_refs_return | e826cc0af1e07211 | ok | ok | ok | ok | [2i8, 3i8] | 190f1c7d145e22f6 |
| nested_arrays_from_brillig | d58f2c9d13d7df77 | ok | ok | ok | n/a: no recorded return | () | 4bdef37a13f76f74 |
| nested_dyn_array_regression_5782 | 30571e21c432c5a7 | ok | ok | ok | n/a: no recorded return | () | 1d4f9395d91484e6 |
| nested_fmtstr | 793d0e4026ec7db2 | ok | ok | FAIL Unsupported(format string interpolating a field): format string interpolating a field | n/a: not interpreted |  | 311e3167836c48a0 |
| nested_if_then_block_same_cond | 5571bdb8856b371f | ok | ok | ok | ok | [true, false] | f6173594b91bca5b |
| nested_vector_last_index_access_post_insert | c4a86338ed959765 | ok | ok | ok | ok | 4u32 | 155b00d4b5494334 |
| nested_vector_pop_back | 9db08d2da61fb84d | ok | ok | ok | n/a: no recorded return | () | 00bad1b29544a995 |
| nested_vector_pop_front_return | fb6d0f77507f3a59 | ok | ok | ok | ok | [[21u32, 22u32, 23u32, 24u32, 25u32], [6u32, 7u32, 8u32, 9u32, 10u32], [11u32, 12u32, 13u32, 14u32, 15u32], …#909bbdf9 | bb275415a184785b |
| nested_vector_push_front_return | 023b5fe0838050b7 | ok | ok | ok | ok | [[21u32, 22u32, 23u32, 24u32, 25u32], [1u32, 2u32, 3u32, 4u32, 5u32], [21u32, 22u32, 23u32, 24u32, 25u32], […#51cae978 | 962090272e1a2d3a |
| nested_vector_return | 96ba22379496bd86 | ok | ok | ok | ok | [[1u32, 2u32, 3u32, 4u32, 5u32], [6u32, 100u32, 8u32, 9u32, 10u32], [11u32, 12u32, 13u32, 14u32, 15u32], [16…#5dbcc768 | eaf36a0d1bf0cfe3 |
| no_predicates_basic | 128ec0fb6ec0d20e | ok | ok | ok | n/a: no recorded return | () | b37ed50f0fd51cc8 |
| no_predicates_brillig | 19b4677197b0ff33 | ok | ok | ok | n/a: no recorded return | () | c65c37287fae03d1 |
| no_predicates_numeric_generic_poseidon | 69634f5815332aeb | ok | FAIL DependencyCompileGap: program reaches dependency code from files that failed elaboration under the chos…#5da1f8a9 | n/a: not compiled | n/a: not interpreted |  |  |
| numeric_type_alias | f8296fdaa3ed1643 | ok | ok | ok | n/a: no recorded return | () | 32f7175cc9476953 |
| op_assign_desugaring | f96c98d8545da3a0 | ok | ok | ok | n/a: no Prover.toml | () | fa364d4e4fe6ef78 |
| overlapping_dep_and_mod | d3e8b232fa58707c | n/a: workspace manifest: the referee runs single-package programs | n/a: not loaded | n/a: not compiled | n/a: not interpreted |  |  |
| pedersen_check | d2420c6ce78c82e9 | ok | FAIL Panic: internal error: entered unreachable code: Encountered Error node during monomorphization | n/a: not compiled | n/a: not interpreted |  |  |
| pedersen_commitment | 8364066eedadbf2e | ok | FAIL Panic: internal error: entered unreachable code: Encountered Error node during monomorphization | n/a: not compiled | n/a: not interpreted |  |  |
| pedersen_hash | 1d9720aa99ef1511 | ok | FAIL Panic: internal error: entered unreachable code: Encountered Error node during monomorphization | n/a: not compiled | n/a: not interpreted |  |  |
| poseidon_bn254_hash_width_3 | e227db45855c1ff8 | ok | FAIL CompileError: Invalid array length | n/a: not compiled | n/a: not interpreted |  |  |
| poseidonsponge_x5_254 | 2f2fe63fa2b934f1 | ok | FAIL CompileError: Could not resolve 'sponge' in path \| The value `3637726918731233354960448572465528704217…#f2bacf0f | n/a: not compiled | n/a: not interpreted |  |  |
| pred_eq | 9c4d5bdcbefc93b8 | ok | ok | ok | n/a: no recorded return | () | 1b565af1c78993c6 |
| primitive_type_alias_method | c2e69ae9dbf92c35 | ok | FAIL DependencyCompileGap: program reaches dependency code from files that failed elaboration under the chos…#90ec0f4a | n/a: not compiled | n/a: not interpreted |  |  |
| print_composite_array | 64f341e550600709 | ok | ok | FAIL Unsupported(oracle call): oracle call 'print' | n/a: not interpreted |  | d9917d7183600483 |
| private_inherent_method_resolves_to_trait | fb623e671cb1e11a | ok | ok | ok | n/a: no recorded return | () | 29723c26e8c7e15b |
| ref_in_unconstrained_function_pointer | 524e252aa8a57c1e | ok | ok | ok | n/a: no recorded return | () | 9c0532ee3bfeb8ab |
| reference_alias_in_array | 62f41bac0bf5afa5 | ok | ok | ok | n/a: no Prover.toml | () | baa485441d357d9f |
| reference_cancelling | c43585b2690d829b | ok | ok | FAIL Unsupported(oracle call): oracle call 'print' | n/a: not interpreted |  | f59b95172f77fe12 |
| reference_counts_inliner_0 | cc866da0b60534fd | ok | ok | FAIL Unsupported(intrinsic): intrinsic 'array_refcount' | n/a: not interpreted |  | debb43c0e400e125 |
| reference_counts_inliner_max | d183b47facb097ce | ok | ok | FAIL Unsupported(intrinsic): intrinsic 'array_refcount' | n/a: not interpreted |  | 128354c645f09825 |
| reference_counts_inliner_min | 1f9ccadfe90ea864 | ok | ok | FAIL Unsupported(intrinsic): intrinsic 'array_refcount' | n/a: not interpreted |  | debb43c0e400e125 |
| reference_counts_vectors_inliner_0 | ba06e266a42974d8 | ok | ok | FAIL Unsupported(intrinsic): intrinsic 'vector_refcount' | n/a: not interpreted |  | 48ca8f6507922a11 |
| reference_only_used_as_alias | 61e3ff80506332ff | ok | ok | FAIL Unsupported(dereference of a non-reference value): dereference of a non-reference value (Tuple([RefCell…#559dd942 | n/a: not interpreted |  | d8c2b778c173afe6 |
| references | 5fb79399c8b42e58 | ok | ok | FAIL Unsupported(dereference of a non-reference value in lvalue): dereference of a non-reference value in lv…#58ef07fa | n/a: not interpreted |  | 1788f8324fe0568c |
| regression_10008 | d7e0d26a3e5233a6 | ok | ok | ok | ok | 1i16 | dc56022e21d93b02 |
| regression_10141 | 19964342ecbca402 | ok | ok | ok | ok | 10 | 3d6cbcacdd9d784a |
| regression_10156 | 2984bb388b772aa2 | ok | ok | FAIL Unsupported(oracle call): oracle call 'print' | n/a: not interpreted |  | f0b1612618982cc0 |
| regression_10158 | 02f5b8115fad61b9 | ok | ok | FAIL Unsupported(oracle call): oracle call 'print' | n/a: not interpreted |  | 8e5b330e7a51ef47 |
| regression_10170 | 76564c5f6ab6751d | ok | ok | ok | n/a: no recorded return | () | afa45b5e581b5030 |
| regression_10180 | e12af6e8c162999c | ok | FAIL CompileError: The value `193432430920915057603408161267722629873` cannot fit into `Field` which has ran…#27d43df8 | n/a: not compiled | n/a: not interpreted |  |  |
| regression_10197 | 34116521b5c35491 | ok | ok | FAIL Unsupported(oracle call): oracle call 'print' | n/a: not interpreted |  | 9b4d24ffccf1a026 |
| regression_10198 | e72c42f2ff9f1c2d | ok | ok | FAIL Unsupported(oracle call): oracle call 'print' | n/a: not interpreted |  | e62a9ed3062253fb |
| regression_10307 | 2ed675327751bd59 | ok | ok | ok | ok | false | fac224f6b75bb94f |
| regression_10446 | 1e5c6dd127b2b45b | ok | ok | ok | ok | true | 50efafa57e3c49d6 |
| regression_10452 | 92d944ff6258311c | ok | ok | ok | n/a: no Prover.toml | () | 6c78f1df3474e111 |
| regression_10466 | 432a5ae0f9d25e7a | ok | ok | FAIL Unsupported(oracle call): oracle call 'print' | n/a: not interpreted |  | 006e54975006a36a |
| regression_10516 | 454c4d5db06e3ee5 | ok | ok | ok | n/a: no recorded return | () | 19c8c1c618d8dce8 |
| regression_10690 | 8533d51264bb8f3e | ok | FAIL DependencyCompileGap: program reaches dependency code from files that failed elaboration under the chos…#90ec0f4a | n/a: not compiled | n/a: not interpreted |  |  |
| regression_10917 | 0f31647fb23e7f8f | ok | ok | ok | ok | true | 25cb23a3ff763330 |
| regression_10923 | 05dc4166f4e7337e | ok | ok | ok | ok | true | 70e67179ec7fd644 |
| regression_10975 | 5de37ceda5ec6e32 | ok | ok | FAIL Unsupported(oracle call): oracle call 'print' | n/a: not interpreted |  | 00441bf0ed6f62a3 |
| regression_10977 | 80fdf9fc540225a7 | ok | ok | FAIL Unsupported(oracle call): oracle call 'print' | n/a: not interpreted |  | 7f9d7d229873f472 |
| regression_11048 | 4d2d8a0bd85abcbe | ok | ok | ok | ok | 3u32 | e9337ee9e4887695 |
| regression_11134 | 7ca9dd29a6020029 | ok | ok | ok | ok | (1, 2) | 80c21c4e04723ce0 |
| regression_11294 | 0820a3e05338c477 | ok | ok | FAIL InputError: failed to parse Prover.toml: The value passed for parameter `previous_kernel_public_inputs.…#06a49d74 | n/a: not interpreted |  | 074711fa4bd695b1 |
| regression_11402 | 9587cafe3147daed | ok | ok | ok | ok | "v*" | 14d8acfc1198b717 |
| regression_11440 | 8be43c9b84c316e7 | ok | ok | FAIL Unsupported(oracle call): oracle call 'print' | n/a: not interpreted |  | 5442aab323b3b8fd |
| regression_1144_1169_2399_6609 | 8ffe70ee1dfaa22e | ok | ok | ok | n/a: no recorded return | () | 70de4d1aa369434b |
| regression_11463 | 1e993dd4c250ef93 | ok | ok | ok | n/a: no Prover.toml | () | 8b67981a1d23df23 |
| regression_11484 | 7650aa32ff4fe1d3 | ok | ok | FAIL Unsupported(oracle call): oracle call 'print' | n/a: not interpreted |  | 1e4a679be01ffb9c |
| regression_11540 | dbfbb71c4cd0c053 | ok | ok | ok | n/a: no Prover.toml | () | 7656cb9c5d9d1f24 |
| regression_11659 | 816cceeb0e4e90dc | ok | FAIL DependencyCompileGap: program reaches dependency code from files that failed elaboration under the chos…#81dd487c | n/a: not compiled | n/a: not interpreted |  |  |
| regression_11889 | e8f312e171b551f1 | ok | ok | FAIL Unsupported(oracle call): oracle call 'print' | n/a: not interpreted |  | 75194ce924d272c7 |
| regression_12034 | a9c189e38c0c08da | ok | FAIL CompileError: The value `340282366920938463463374607431768211456` cannot fit into `Field` which has ran…#91880a86 | n/a: not compiled | n/a: not interpreted |  |  |
| regression_12149 | fbe313666373999e | ok | ok | ok | ok | 10u32 | 6e6a46d6701797c2 |
| regression_12269 | 2b2d2998480c1be3 | ok | ok | FAIL Unsupported(non-UTF-8 string literal): non-UTF-8 string literal: invalid utf-8 sequence of 1 bytes from index 2 | n/a: not interpreted |  | 2d1dd2255def43e6 |
| regression_12317 | e1d244dfdb1d6c09 | ok | ok | ok | ok | 1 | 120ebfba4639cf9c |
| regression_12467 | 0dffa97d908afa8a | ok | ok | ok | ok | 4u32 | 0324d8226d375db0 |
| regression_12467_2 | 5b2400d3fb33dab9 | ok | ok | ok | n/a: no recorded return | () | fc920a9c3216f087 |
| regression_12468 | a3887efab9d671a8 | ok | ok | ok | n/a: no recorded return | () | cadbe6cbc5730387 |
| regression_12472 | de0f77b4cafd9af1 | ok | ok | ok | ok | 3u32 | f4726ec4fba0fa4a |
| regression_12473 | 0daae513ebea0d8b | ok | ok | ok | n/a: no recorded return | () | ed189a58474f3cc8 |
| regression_12475 | 58720e2ed42ad136 | ok | ok | ok | n/a: no Prover.toml | () | 407da006a96b7b1c |
| regression_12494 | 5bd50ed5d3e2dd30 | ok | ok | ok | ok | false | 7dc02bb6cc54b173 |
| regression_12572 | a70beba9fd8d4fed | ok | FAIL CompileError: No matching impl found for `Bundle: Default` | n/a: not compiled | n/a: not interpreted |  |  |
| regression_12713 | a3b4b35ce67c3793 | ok | ok | FAIL Unsupported(oracle call): oracle call 'print' | n/a: not interpreted |  | b3761b7d3e27acef |
| regression_13040 | b331d94f66212b92 | ok | ok | ok | ok | 99 | 8858a810f59cdf3b |
| regression_1457_array_set_nested_shared | 0f0de9ee271fad23 | ok | ok | ok | ok | ([[0]], [7]) | bc450e918f51e281 |
| regression_2660 | 0dee9789df49c4ee | ok | ok | ok | n/a: no recorded return | () | cdd2d1aaeb0caf42 |
| regression_3051 | 32c2861484b93f27 | ok | ok | FAIL Unsupported(oracle call): oracle call 'print' | n/a: not interpreted |  | 4d1e95af8d1e7027 |
| regression_3394 | 1046db2f9201fea2 | ok | ok | FAIL Unsupported(oracle call): oracle call 'print' | n/a: not interpreted |  | 53ef15d4392742f5 |
| regression_3607 | 1664e14fd846e721 | ok | ok | ok | n/a: no recorded return | () | bc46e4f303fdfb27 |
| regression_3889 | 2dd7bc5b00edc1f4 | ok | ok | ok | ok | 18 | 4a0ca47e1f7594f0 |
| regression_4088 | 26192c4994543e95 | ok | ok | ok | n/a: no recorded return | () | b98c68d4a65237b4 |
| regression_4124 | a3622338dd43f002 | ok | ok | ok | n/a: no recorded return | () | 70bd95727287da9f |
| regression_4202 | 2a0faa2501bb600b | ok | ok | ok | n/a: no recorded return | () | 6b7ef72094bd9ac3 |
| regression_4449 | 15dcdfc8d271c5c6 | ok | FAIL DependencyCompileGap: program reaches dependency code from files that failed elaboration under the chos…#81dd487c | n/a: not compiled | n/a: not interpreted |  |  |
| regression_4663 | 9760d98c0ad87a96 | ok | FAIL DependencyCompileGap: program reaches dependency code from files that failed elaboration under the chos…#81dd487c | n/a: not compiled | n/a: not interpreted |  |  |
| regression_4709 | a3a1e9cb23f45581 | ok | FAIL Panic: internal error: entered unreachable code: Encountered Error node during monomorphization | n/a: not compiled | n/a: not interpreted |  |  |
| regression_5045 | affc6dae5842fe74 | ok | FAIL CompileError: The value `13369208703810469428642933335211858179748250278026939592718845896191201195622`…#35da11ae | n/a: not compiled | n/a: not interpreted |  |  |
| regression_5252 | f3a02d6e57598305 | ok | FAIL CompileError: Could not resolve 'sponge' in path | n/a: not compiled | n/a: not interpreted |  |  |
| regression_5435 | 6d8a6e68f20296dc | ok | ok | ok | n/a: no recorded return | () | c2efb9ea8241ac6e |
| regression_5615 | 00dd4342478475da | ok | FAIL Panic: internal error: entered unreachable code: Encountered Error node during monomorphization | n/a: not compiled | n/a: not interpreted |  |  |
| regression_6285 | d0f8eea2a8212fe3 | ok | ok | FAIL Unsupported(oracle call): oracle call 'print' | n/a: not interpreted |  | db36e114a1e263b6 |
| regression_6451 | 96e7e5f294138d1b | ok | FAIL DependencyCompileGap: program reaches dependency code from files that failed elaboration under the chos…#90ec0f4a | n/a: not compiled | n/a: not interpreted |  |  |
| regression_6674_1 | 61d325fa07a9ce10 | ok | ok | FAIL Unsupported(dereference of a non-reference value in lvalue): dereference of a non-reference value in lv…#d81bcc97 | n/a: not interpreted |  | 884c66f181bcb433 |
| regression_6674_2 | a9b5d5b3da3eb5e1 | ok | ok | FAIL Unsupported(dereference of a non-reference value in lvalue): dereference of a non-reference value in lv…#d81bcc97 | n/a: not interpreted |  | fb1e7d7536ebccb7 |
| regression_6674_3 | ecb7dcadf436e616 | ok | ok | FAIL Unsupported(dereference of a non-reference value): dereference of a non-reference value (Tuple([RefCell…#1c96616d | n/a: not interpreted |  | 6644168a52ecf1d4 |
| regression_6734 | 02cceee4f48167b3 | ok | ok | ok | n/a: no Prover.toml | () | daeebc9dfe9d49c6 |
| regression_6834 | 9d494e4a5ad1c873 | ok | ok | ok | ok | 0u32 | 82ef7af14f4ba3a2 |
| regression_6990 | 32d232bdce8d1ec7 | ok | ok | FAIL Unsupported(oracle call): oracle call 'print' | n/a: not interpreted |  | 50ea07c5276acfab |
| regression_7062 | 8813544697902517 | ok | ok | ok | n/a: no recorded return | () | 0544db01bafd5205 |
| regression_7128 | c89006da86215b24 | ok | FAIL Panic: internal error: entered unreachable code: Encountered Error node during monomorphization | n/a: not compiled | n/a: not interpreted |  |  |
| regression_7143 | 53e4e6265d13cf0d | ok | ok | ok | ok | true | 068f7e93643ab1b2 |
| regression_7195 | 76007851a9749d3a | ok | ok | ok | n/a: no recorded return | () | fd0688f25d60b281 |
| regression_7323 | 7f94696048e93836 | ok | ok | ok | n/a: no recorded return | () | 85d04d1672693a10 |
| regression_7451 | 09768c1f099861e7 | ok | ok | ok | n/a: no recorded return | () | 36f4fe4cf2f44345 |
| regression_7612 | 6fb8aafcd099dee8 | ok | ok | ok | n/a: no recorded return | () | 618996449edc7f8c |
| regression_7744 | 860dbb7dba09d600 | ok | FAIL DependencyCompileGap: program reaches dependency code from files that failed elaboration under the chos…#6469ab74 | n/a: not compiled | n/a: not interpreted |  |  |
| regression_7836 | 6c85de02ffac8cf6 | ok | ok | ok | n/a: no recorded return | () | cbd897959a3c6dc4 |
| regression_7962 | dbba6bf4733d5bb0 | ok | ok | FAIL InputError: failed to parse Prover.toml: The value passed for parameter `v0` is invalid: Value 18446744…#d6a04f0c | n/a: not interpreted |  | a76c233bf316c04b |
| regression_8009 | 718067b1fd319a55 | ok | ok | FAIL Unsupported(signed 64-bit ABI input is not representable in this field): signed 64-bit ABI input is not…#2a723acd | n/a: not interpreted |  | 438a00a7d3aa7345 |
| regression_8011 | a35403014f7fc0fc | ok | ok | ok | ok | 15u32 | a0fe1b3d8401b56b |
| regression_8174 | 6e0d3070ff8dd4ec | ok | ok | ok | ok | [["LNV", "ZIH"]] | 43ff0a654a675eda |
| regression_8210 | d6055ba4f40e61df | ok | ok | ok | n/a: no Prover.toml | () | 4fb736eac4ae0451 |
| regression_8212 | 4e3633b4a7541ab5 | ok | ok | ok | n/a: no recorded return | () | 1340903f2e8e82e1 |
| regression_8235 | f1fd11bf3b5a6357 | ok | ok | ok | ok | false | c3b0d3643e69c1a1 |
| regression_8236 | a2c0a5c8c17e8518 | ok | ok | ok | ok | false | 4597a7a798721106 |
| regression_8261 | 50904386b81623c8 | ok | ok | FAIL InputError: failed to parse Prover.toml: The value passed for parameter `c[0][0]` is invalid: Value 0x1…#f62db2c3 | n/a: not interpreted |  | 92d557c61fb9fbb0 |
| regression_8305 | b5a584a93ddfb06a | ok | ok | ok | ok | -2i32 | 5c85f0791392a8b5 |
| regression_8329 | e002b316e71904cd | ok | ok | ok | ok | 1u8 | 726f5e848781f38d |
| regression_8519 | a36b8352dde0c45f | ok | ok | FAIL InputError: failed to parse Prover.toml: The value passed for parameter `return` is invalid: Value 5343…#2becf9a6 | n/a: not interpreted |  | 64a0368f485edfc1 |
| regression_8558 | 44a60bd62fb123fb | ok | ok | ok | ok | 0u8 | 3d83e9f5c88450c3 |
| regression_8662 | 2f7edcf0e8a252d4 | ok | ok | ok | ok | true | b03b34dba6f45347 |
| regression_8726 | 81f4ac3145211d10 | ok | ok | FAIL Unsupported(signed 64-bit ABI input is not representable in this field): signed 64-bit ABI input is not…#2a723acd | n/a: not interpreted |  | 095d9536f1f97ed1 |
| regression_8729 | d416212cf076a13a | ok | ok | FAIL Unsupported(oracle call): oracle call 'print' | n/a: not interpreted |  | b8a627c7ce0e27ef |
| regression_8739 | 5c49c445e5a59c18 | ok | ok | ok | n/a: no Prover.toml | () | d2088ab73c034ca2 |
| regression_8755 | 6bb47f007cc0a3ff | ok | FAIL CompileError: The value `157653526363045079447323020681982670581` cannot fit into `Field` which has ran…#74ed7640 | n/a: not compiled | n/a: not interpreted |  |  |
| regression_8761 | 8b63e9d15aff900e | ok | ok | ok | ok | 1 | 23d9e36fae14b320 |
| regression_8779 | ab652448e209f177 | ok | ok | ok | ok | true | 4c677182e5f9c6ed |
| regression_8874 | 9318ecf349a050b1 | ok | ok | ok | n/a: no recorded return | () | d36e2b5a3c09daa7 |
| regression_8890 | 00f5c55a72d6b973 | ok | ok | ok | ok | 5 | 00bb8bf38f422308 |
| regression_8926 | 4c9446101b24a0dc | ok | ok | ok | ok | "ABC" | 07828aa6aa76743c |
| regression_8975 | 3ad30ad5f0892460 | ok | ok | ok | ok | true | 47ce68c08bd5d655 |
| regression_8980 | 545d521671d93739 | ok | ok | ok | ok | true | ff52373c1fc675a9 |
| regression_9037 | 72ac6c21b19d2376 | ok | ok | ok | ok | [[false]] | 4e727208e545ebfe |
| regression_9047 | 24cd1140456bd456 | ok | ok | ok | ok | "ok" | 2b961c68b59ee2f6 |
| regression_9102 | 0fdd97e9b77f0c62 | ok | ok | ok | ok | [true, false] | 846ac1d5e98b9caa |
| regression_9116 | dec53bd99810fec9 | ok | FAIL CompileError: No method named 'serialize' found for type 'Foo' \| No matching impl found for `Foo: Dese…#4260360a | n/a: not compiled | n/a: not interpreted |  |  |
| regression_9119 | ed70a7249553a7ee | ok | ok | ok | ok | true | c4a76ffa5a4852c3 |
| regression_9160 | 65221ba51529288a | ok | ok | FAIL Unsupported(oracle call): oracle call 'print' | n/a: not interpreted |  | cdd393bfbbf845fd |
| regression_9193 | aa2beadde87f6e1d | ok | ok | ok | n/a: no recorded return | () | ac9dd01cc716d809 |
| regression_9206 | 7eb9ed595e36907d | ok | ok | ok | ok | 0u32 | b59ef6f27c6f47e6 |
| regression_9208 | 8cdb498f1b728176 | ok | ok | ok | ok | 18446744069414584320 | 44d9984e812b8fd2 |
| regression_9243 | 73982b1a1c689ad3 | ok | ok | ok | n/a: no recorded return | () | 049e2146c2f95c36 |
| regression_9271 | 75dc4f1c29970244 | ok | ok | FAIL Unsupported(oracle call): oracle call 'print' | n/a: not interpreted |  | 1d7ba8286f5a1024 |
| regression_9294 | 91c1136dd2278664 | ok | ok | FAIL Unsupported(oracle call): oracle call 'print' | n/a: not interpreted |  | a5f317c0957cec5e |
| regression_9303 | d41e3fe08992d839 | ok | ok | ok | ok | (18446744069414584320, true) | 6bac375473fed87e |
| regression_9312 | 862c55cf8c1f46cc | ok | ok | ok | ok | ((1u32, 20), (3u32, 60)) | 6730ab950c189407 |
| regression_9329 | 364f8e3354746b48 | ok | ok | ok | ok | 21i8 | a867f86dad824ecb |
| regression_9415 | 8fdb228672fd3826 | ok | ok | ok | n/a: no recorded return | () | 70189bc44e21446f |
| regression_9439 | 148bb7478d76f82e | ok | ok | ok | n/a: no recorded return | () | 2564de88fb67cb48 |
| regression_9455 | 4411279f30d03d80 | ok | ok | ok | n/a: no recorded return | () | b04e6936347142aa |
| regression_9467 | 446fcc186dffc9ed | ok | ok | ok | ok | (1u32, 2u32) | 9b1890f0d2d3fea5 |
| regression_9496 | 8bd1827e64f215d5 | ok | ok | FAIL Unsupported(oracle call): oracle call 'print' | n/a: not interpreted |  | 9cd3a05498220447 |
| regression_9538 | cbf3393729b3d394 | ok | ok | ok | ok | "two" | cf7ce89d87a1df1f |
| regression_9541 | 12290bc42812064e | ok | ok | ok | ok | 67108864u32 | e2df71e8a000c5a4 |
| regression_9544 | 81bce0baf9ecad69 | ok | ok | ok | ok | 5u64 | 4885c3efa8852d22 |
| regression_9546 | 330b4937d34089a4 | ok | ok | ok | ok | -8670i16 | 1c4a6983b7801999 |
| regression_9578 | a0dd1c41742c4f2d | ok | ok | FAIL Unsupported(oracle call): oracle call 'print' | n/a: not interpreted |  | 138193e58a29f6f3 |
| regression_9593 | 411383562e2595bc | ok | ok | ok | ok | "Cc" | 4c4d91fce555ffd4 |
| regression_9594 | bc29084a887e7590 | ok | ok | ok | ok | 2i32 | d1bf8013e4f3b6a4 |
| regression_9657 | a23c53e1c7c312ae | ok | ok | ok | n/a: no recorded return | () | 9168905c68bffc5c |
| regression_9725_1 | 7d622c0c59fcf2b7 | ok | ok | ok | n/a: no Prover.toml | () | b27badce41b01715 |
| regression_9725_2 | b2691069f029373f | ok | ok | ok | n/a: no Prover.toml | () | a386595263662e64 |
| regression_9758 | a3d2ddba0e2c70c1 | ok | ok | ok | ok | true | d886db988d2a6c66 |
| regression_9764 | a2732df45a05a5ac | ok | ok | ok | ok | 0 | 79fb87ab8b8db5bd |
| regression_9804 | 424e2864bcc7a9bc | ok | ok | ok | ok | 0 | f81187f7b5434aa2 |
| regression_9860 | a2e38534aa2508f9 | ok | ok | ok | ok | [[0u32, 0u32, 2u32, 3u32], [1u32, 2u32, 3u32, 0u32], [2u32, 0u32, 0u32, 1u32], [3u32, 0u32, 1u32, 2u32]] | 9193823ba9b27d55 |
| regression_9888 | 8b364b39aa6e4dc7 | ok | FAIL CompileError: The value `48908553793922955213460396216351077763` cannot fit into `Field` which has rang…#2a5434c8 | n/a: not compiled | n/a: not interpreted |  |  |
| regression_9907 | cf5b5317d25e481b | ok | ok | ok | ok | [[3405691582]] | f52a3ff8c9f2bb8b |
| regression_9971 | 0f5a222e004a7675 | ok | ok | ok | n/a: no recorded return | () | eaf688166fac75a2 |
| regression_brillig_const_fold_self_dedup | 7adeb511e1c7680f | ok | ok | FAIL InputError: failed to parse Prover.toml: The value passed for parameter `b` is invalid: Value 218882428…#14d15503 | n/a: not interpreted |  | de054ba4fb21147b |
| regression_brillig_ref_deref_crash | 403947a9c0313521 | ok | ok | ok | ok | [0] | 394f48099f87be7e |
| regression_capacity_tracker | 2cc2d40bae2c8ea5 | ok | ok | ok | n/a: no recorded return | () | 0fe1aa2ba8d64db3 |
| regression_claude_1019 | 53f964e7d13f8f5c | ok | FAIL DependencyCompileGap: program reaches dependency code from files that failed elaboration under the chos…#90ec0f4a | n/a: not compiled | n/a: not interpreted |  |  |
| regression_claude_1124 | 9602f0001e403454 | ok | ok | ok | ok | 33u32 | 5f51f1c26ea791e8 |
| regression_claude_1201 | 2eea41dec11c4b2a | ok | ok | ok | ok | [99u8, 98u8] | 2e2eef7651b48195 |
| regression_dominated_truncate | 87bc7c5c6aee68df | ok | FAIL DependencyCompileGap: program reaches dependency code from files that failed elaboration under the chos…#90ec0f4a | n/a: not compiled | n/a: not interpreted |  |  |
| regression_field_div_truncate | 4e623b6e35a98851 | ok | FAIL Panic: internal error: entered unreachable code: Encountered Error node during monomorphization | n/a: not compiled | n/a: not interpreted |  |  |
| regression_foreign_proxy_generic | ff2bc670903f3fc9 | ok | ok | FAIL Unsupported(intrinsic): intrinsic 'blake2s' | n/a: not interpreted |  | bf506d91b18e4b5d |
| regression_inner_if_else_collapse | 977def12c02bb3a1 | ok | ok | ok | ok | (false, true) | ae562b3638fe95a4 |
| regression_licm_induction_var | e420c51271492327 | ok | ok | ok | n/a: no recorded return | () | 11cae2b4dac212a5 |
| regression_loop_unroll_header_instructions | 367d2c14aca488f0 | ok | ok | ok | n/a: no recorded return | () | 2f953d57e9533c58 |
| regression_mem2reg_make_array_of_refs | b2093d52ea9e8cfd | ok | ok | ok | ok | true | a949a2244980f32e |
| regression_mem2reg_unknown_array_aliases | c2cbbc2c3fda37a7 | ok | ok | ok | ok | true | b40edac4d3fb416e |
| regression_mem_op_predicate | 50a548438dabbdea | ok | ok | ok | n/a: no recorded return | () | d92104263c69c2d6 |
| regression_method_cannot_be_found | 28d25360836b9349 | ok | ok | FAIL Unsupported(oracle call): oracle call 'print' | n/a: not interpreted |  | 4dc24e841e1b447e |
| regression_nc1436 | 3eac5a051b8026f8 | ok | ok | FAIL Unsupported(break/continue in value position): break/continue in value position | n/a: not interpreted |  | 87b77683069601d4 |
| regression_noir_claude_1069 | b7dc519741e245bc | ok | ok | ok | n/a: no recorded return | () | 9ae342f68400fd71 |
| regression_noir_claude_1365 | 648df2c56eeaf2d7 | ok | ok | ok | ok | 0u32 | a057601c7c274573 |
| regression_nrsec_903 | fb527083a5fb0261 | ok | ok | ok | ok | 100u32 | be42f3e4875885e5 |
| regression_oob_constant_tuple_array_get | 12e33146ff200977 | ok | ok | ok | ok | 0 | dd60054d49526cce |
| regression_struct_array_conditional | b4c348bc576a0bba | ok | ok | FAIL InputError: failed to parse Prover.toml: The value passed for parameter `x[0].value` is invalid: Value …#1183f8d4 | n/a: not interpreted |  | 7f0fdff866eae353 |
| regression_truncate_unchecked_sub | 7c9242f9acdb1099 | ok | ok | ok | ok | 0u32 | c280f429ec298cf5 |
| regression_unroll_body_break | e825c82b010df0df | ok | ok | ok | n/a: no Prover.toml | () | 9d4c34c9004887f2 |
| regression_unsafe_no_predicates | f2e607c55e9cea5e | ok | ok | ok | n/a: no recorded return | () | 619cf0b1b7eb51d1 |
| regression_unused_nested_array_get | d109c7dfd1e63b71 | ok | FAIL CompileError: -244112236639376348452645045105852040922 is outside the range of the Field type \| The va…#fefdc19e | n/a: not compiled | n/a: not interpreted |  |  |
| regression_while_condition_alias | 96b027c5201932bf | ok | ok | ok | ok | 5 | e99a7b30f4fe3a92 |
| regression_while_condition_break | e202777b5d8afaa3 | ok | ok | FAIL Unsupported(break/continue in value position): break/continue in value position | n/a: not interpreted |  | 7c361db4a05450cc |
| return_twice | 88474269c0d2b4f4 | ok | ok | ok | ok | (100, 100) | 48afe550bd842a9b |
| shift_left_rhs_value_casted_from_smaller_type | d7b82b9b8ed1648b | ok | ok | ok | ok | 7435510297333629952u64 | 16a63cfe5794c897 |
| shift_right_overflow | e9dd3179d62e1fcb | ok | ok | ok | n/a: no recorded return | () | 822e8c4e50b08277 |
| shl_signed_regression_9661 | 9d5a0639b27d72b4 | ok | ok | ok | ok | 7i8 | c930fc100450672b |
| side_effects_constrain_array | b961e60f12e2205e | ok | ok | ok | n/a: no recorded return | () | 3d22a08af64d5576 |
| signed_arithmetic | 71765f02593e5966 | ok | ok | ok | n/a: no recorded return | () | bfd28e2e5e8890c4 |
| signed_bitshift | 12903e517afb573c | ok | ok | ok | n/a: no Prover.toml | () | 1b9f8c0ecfa8edcc |
| signed_cmp | ba9f87002f31b66a | ok | ok | ok | n/a: no recorded return | () | 4b00535c175b046b |
| signed_comparison | 80c3ee504f8e49eb | ok | ok | ok | n/a: no recorded return | () | 2afe2e61edafca63 |
| signed_div | bb9fcac815f838a8 | ok | ok | ok | n/a: no recorded return | () | 9e89d31b96b6bd48 |
| signed_division | f09b46f2f9e28980 | ok | FAIL DependencyCompileGap: program reaches dependency code from files that failed elaboration under the chos…#d581b3ff | n/a: not compiled | n/a: not interpreted |  |  |
| signed_inactive_division_by_zero | f97d74cdaa6086d0 | ok | ok | ok | ok | 0i32 | fae7a3939fc8dcb3 |
| signed_integer_or_max | 7cd95255f004766f | ok | ok | ok | n/a: no recorded return | () | a9b9ff638145980a |
| signed_overflow_in_else_regression_8617 | 0dd3904473090e8d | ok | ok | ok | ok | -771254105i32 | 94bd9c38b9066c4e |
| signed_truncation | 1f6b77bd96424357 | ok | ok | FAIL Unsupported(signed 64-bit ABI input is not representable in this field): signed 64-bit ABI input is not…#2a723acd | n/a: not interpreted |  | 48c32894326ca13e |
| simple_2d_array | e0708d60e10dd43c | ok | ok | ok | n/a: no recorded return | () | a5f0fba74edb60b6 |
| simple_add_and_ret_arr | 0e9fa11d3150e210 | ok | ok | ok | ok | [2] | 412ad9460a5e17af |
| simple_array_param | d112a48366350e0a | ok | ok | ok | ok | 1 | a43ff568268d30e5 |
| simple_bitwise | d78a5cefc9955972 | ok | ok | ok | ok | 24u8 | 41eef0c366a18f27 |
| simple_comparison | adb2758638451604 | ok | ok | ok | n/a: no recorded return | () | 8f3178fae5f23e96 |
| simple_mut | e080338d0b0fe40d | ok | ok | ok | ok | 3 | c6046ebba3fd5856 |
| simple_not | 7fdc4e5fccad23e9 | ok | ok | ok | ok | true | 84ee5a3aa3cc308f |
| simple_print | e4df0206c03744a8 | ok | ok | FAIL Unsupported(oracle call): oracle call 'print' | n/a: not interpreted |  | bf127751db85213c |
| simple_program_addition | 0700f294cd267230 | ok | ok | ok | ok | 4 | 8946b15116cd65c0 |
| simple_radix | 59b453508526756f | ok | FAIL DependencyCompileGap: program reaches dependency code from files that failed elaboration under the chos…#90ec0f4a | n/a: not compiled | n/a: not interpreted |  |  |
| simple_shield | 326a192bd14e0a38 | ok | FAIL Panic: internal error: entered unreachable code: Encountered Error node during monomorphization | n/a: not compiled | n/a: not interpreted |  |  |
| simple_shift_left_right | b2daffcbe65bd19b | ok | ok | ok | n/a: no recorded return | () | 1c0df4270c8d0a68 |
| static_assert_empty_loop | b9640965494b1149 | ok | ok | FAIL Unsupported(oracle call): oracle call 'print' | n/a: not interpreted |  | 5ef11ec653db58c6 |
| stored_unconstrained_fn_regression | c8382350801c3013 | ok | ok | ok | n/a: no recorded return | () | 754b15fe56291263 |
| strings | 78f01f0e32e7af4f | ok | FAIL Panic: internal error: entered unreachable code: Encountered Error node during monomorphization | n/a: not compiled | n/a: not interpreted |  |  |
| struct | f16ea0d9aecc973b | ok | ok | ok | n/a: no recorded return | () | 58ab1efbe6097d26 |
| struct_array_inputs | 3742ab885d12a073 | ok | ok | ok | ok | 3 | 5eca369524e1961b |
| struct_assignment_with_shared_ref_to_field | 6e781f7fdc4ed117 | ok | ok | ok | n/a: no Prover.toml | () | af956faea5509df8 |
| struct_fields_ordering | c015ece6f4b998a6 | ok | ok | ok | n/a: no recorded return | () | 804d41a3cb59e9e7 |
| struct_inputs | 92a033dd1b265e48 | ok | ok | ok | ok | 1 | eeb3155e95b5ab7f |
| submodules | a43b2f5024068788 | ok | ok | ok | n/a: no recorded return | () | e3e81d7c5ab2e4cc |
| to_be_bytes | bf46432832001be1 | ok | FAIL DependencyCompileGap: program reaches dependency code from files that failed elaboration under the chos…#90ec0f4a | n/a: not compiled | n/a: not interpreted |  |  |
| to_bytes_consistent | 320aa17e871a3e4b | ok | FAIL DependencyCompileGap: program reaches dependency code from files that failed elaboration under the chos…#90ec0f4a | n/a: not compiled | n/a: not interpreted |  |  |
| to_bytes_integration | 46a465edbb2b364a | ok | FAIL DependencyCompileGap: program reaches dependency code from files that failed elaboration under the chos…#90ec0f4a | n/a: not compiled | n/a: not interpreted |  |  |
| to_le_bytes | 2c568cb6fda070d1 | ok | FAIL DependencyCompileGap: program reaches dependency code from files that failed elaboration under the chos…#90ec0f4a | n/a: not compiled | n/a: not interpreted |  |  |
| trait_as_return_type | 8b4207cc01420907 | ok | ok | ok | n/a: no recorded return | () | f9bbbbe30c109489 |
| trait_associated_constant | cf0a895fad91b3c4 | ok | ok | ok | n/a: no Prover.toml | () | ee3045fb07ca2e86 |
| trait_impl_base_type | a430653e5b49063a | ok | ok | FAIL Unsupported(format string interpolating a field): format string interpolating a field | n/a: not interpreted |  | 2397d4c35c433ec0 |
| traits_in_crates_1 | 3f4013bdc759e2c7 | ok | ok | ok | n/a: no recorded return | () | 534eff3eecd67928 |
| traits_in_crates_2 | 0f46bdf04a37f550 | ok | ok | ok | n/a: no recorded return | () | 534eff3eecd67928 |
| tuple_inputs | f44125487614bdfe | ok | ok | ok | ok | (2, 1u8) | 8100ade9d0a726d4 |
| tuples | 6d3b88ab88d036e2 | ok | ok | ok | n/a: no recorded return | () | 140fb17776c8648f |
| two_array_chain_mutation | dcf27a1a7b328be4 | ok | ok | ok | n/a: no recorded return | () | 0eb8ff2296565972 |
| type_aliases | 59062a973878cf26 | ok | ok | ok | n/a: no recorded return | () | b14887e9bcbabd1b |
| u128_type | 5dfd2fd9ac98420b | ok | ok | ok | n/a: no recorded return | () | 6a6b0c375a6707e8 |
| u16_support | 852d9d92f2c22712 | ok | ok | ok | n/a: no recorded return | () | b1643528535e81fa |
| uhashmap | 1c3ed2f6de26e920 | ok | FAIL Panic: internal error: entered unreachable code: Encountered Error node during monomorphization | n/a: not compiled | n/a: not interpreted |  |  |
| unary_operator_overloading | e310d7f8edc2d2fb | ok | ok | ok | n/a: no recorded return | () | 37dd26488cb6b526 |
| unroll_loop_header_result_used_after_loop | 1f555e424e071a91 | ok | ok | FAIL Unsupported(oracle call): oracle call 'print' | n/a: not interpreted |  | 70212310fd84b37e |
| unroll_loop_regression | 6cc0d0d90731ee83 | ok | ok | ok | ok | [36, 136] | 013efae9be3dc514 |
| unrolling_regression_8333 | 906f824b5caf0687 | ok | FAIL DependencyCompileGap: program reaches dependency code from files that failed elaboration under the chos…#90ec0f4a | n/a: not compiled | n/a: not interpreted |  |  |
| unsafe_range_constraint | e744e032d2399fdb | ok | FAIL DependencyCompileGap: program reaches dependency code from files that failed elaboration under the chos…#90ec0f4a | n/a: not compiled | n/a: not interpreted |  |  |
| unsigned_to_signed_cast | ae784f625ac0e48e | ok | ok | FAIL InputError: failed to parse Prover.toml: The value passed for parameter `x` is invalid: Value 0xfffffff…#cedafe45 | n/a: not interpreted |  | de2befa36c799ea8 |
| vector_coercion | b94a0435bd249a76 | ok | ok | FAIL Unsupported(oracle call): oracle call 'print' | n/a: not interpreted |  | 960470052d2402c2 |
| vector_dynamic_index | f47f1f84319cfa6b | ok | ok | ok | n/a: no recorded return | () | faebfcf9765d5b98 |
| vector_dynamic_insert | 4c91192697f7f58c | ok | ok | ok | n/a: no recorded return | () | 48b01a5e4d6efef6 |
| vector_insert_after_dynamic_read | 99614fb3e647b1b5 | ok | ok | ok | n/a: no recorded return | () | 0352ef4a9b3f37b4 |
| vector_insert_empty_oob | 52d955722dbf2d59 | ok | ok | FAIL Unsupported(oracle call): oracle call 'print' | n/a: not interpreted |  | 5629c18b0d50db0e |
| vector_insert_empty_oob_2 | a6ed3583cd7fc059 | ok | ok | ok | n/a: no recorded return | () | c0467aea1c5793b9 |
| vector_insert_oob | b3be7dbd6dbba51f | ok | ok | ok | n/a: no recorded return | () | 8ca15a2f0f630be3 |
| vector_insert_oob_index_invalid_pred | 6b1285267f1224a7 | ok | ok | ok | ok | 3u32 | e5136e3729a981cf |
| vector_insert_oob_invalid_pred | 285422891a6f83b8 | ok | ok | ok | n/a: no recorded return | () | a62deb1f8038956a |
| vector_loop | 3794d4815e5e0583 | ok | ok | ok | n/a: no recorded return | () | 31ac09f1ccd1e3eb |
| vector_pop_back_oob_invalid_pred | ccd9a33f264ff478 | ok | ok | ok | n/a: no recorded return | () | 139cbff30f894ad1 |
| vector_pop_back_remove_if_else_bug | 77d4bb5af8ee157d | ok | ok | ok | ok | 0 | 1c9a8ee313fe71c9 |
| vector_pop_back_simplify | bbfa4db404086cc0 | ok | ok | ok | ok | ((10u32, 4u32), 1u32) | 0c9b7084ca22718a |
| vector_pop_front_aliased_source | 3bf8260857d51091 | ok | ok | FAIL Unsupported(oracle call): oracle call 'print' | n/a: not interpreted |  | 753ac61d4c7cc183 |
| vector_pop_front_oob_invalid_pred | 7cc72bfc38a32237 | ok | ok | ok | n/a: no recorded return | () | 331e3d10ede76cef |
| vector_pop_temporary_alias | 6dbe4a8dae27bdee | ok | ok | ok | n/a: no Prover.toml | () | 5c69760b03c324a6 |
| vector_push_back_remove_if_else_bug | edb7c2e12ce255ad | ok | ok | ok | n/a: no recorded return | () | 18d2fc83538c6943 |
| vector_regex | b8a11adaea345689 | ok | ok | FAIL Unsupported(oracle call): oracle call 'print' | n/a: not interpreted |  | ca19eb92f4f7c02c |
| vector_remove_oob_index_invalid_pred | b2a3d9aa20917f00 | ok | ok | ok | ok | 3u32 | acbaf9d3f7cf8ae1 |
| vector_remove_oob_invalid_pred | b4f75f55c43d0b70 | ok | ok | ok | n/a: no recorded return | () | 59db9f8c827ba331 |
| vectors | f3b5fccde75c3393 | ok | FAIL DependencyCompileGap: program reaches dependency code from files that failed elaboration under the chos…#90ec0f4a | n/a: not compiled | n/a: not interpreted |  |  |
| while_cond_clone_regression | a19897846fe8ec56 | ok | ok | ok | n/a: no Prover.toml | () | da662ca2a8cbddd8 |
| while_loop_break_regression_8521 | 29b0db547eef32ce | ok | ok | ok | ok | "SQF" | 249e6218e7674519 |
| wildcard_type | 70432f1c7c56da14 | ok | ok | ok | ok | [7, 14, 10, 6] | d587141f8d7bfa16 |
| witness_compression | 6ae2f86c99c8e0b5 | ok | ok | ok | ok | 3 | dd447dbace46ff01 |
| workspace | 0239f291788c625f | n/a: workspace manifest: the referee runs single-package programs | n/a: not loaded | n/a: not compiled | n/a: not interpreted |  |  |
| workspace_default_member | cf3623b936cfcf82 | n/a: workspace manifest: the referee runs single-package programs | n/a: not loaded | n/a: not compiled | n/a: not interpreted |  |  |
| wrapping_operations | 45ced85dbc5cf04b | ok | FAIL DependencyCompileGap: program reaches dependency code from files that failed elaboration under the chos…#d581b3ff | n/a: not compiled | n/a: not interpreted |  |  |
| xor | 80191effa7fa803f | ok | ok | ok | n/a: no recorded return | () | 15fb00fa5b7c6d67 |
| zeroed_array_of_references | 7dbf6c558fb709fb | ok | ok | ok | n/a: no Prover.toml | () | 245d6ac68f1b6891 |

## Fixtures

| program | source | load | compile | interpret | oracle | return | projection |
| --- | --- | --- | --- | --- | --- | --- | --- |
| interp_aggregate_eq | d27320650269fa05 | ok | FAIL CompileError: No matching impl found for `Point: Eq` \| No matching impl found for `Point: Eq` | n/a: not compiled | n/a: not interpreted |  |  |
| interp_basic | 3107274ffaf27bb0 | ok | ok | ok | n/a: no Prover.toml | () | c3881919fe5b115d |
| interp_closures | e2d2c11af5529046 | ok | ok | ok | n/a: no Prover.toml | () | 6cae922a5d9f3f2b |
| interp_inputs_i32 | d0ff7bf2b6f1faf7 | ok | ok | ok | n/a: no recorded return | -121i32 | ce2b3565c8593605 |
| interp_inputs_mixed | 67e3dbb74ee8b1c8 | ok | ok | ok | n/a: no recorded return | 295u32 | e34351c525cd53fa |
| interp_inputs_struct | 3a7dd6ddfc650d45 | ok | ok | ok | n/a: no recorded return | 3706u32 | 6ca342c0eb983cf9 |
| interp_inputs_u64 | 39afd1992f111d0a | ok | ok | ok | n/a: no recorded return | 18446744069414584328u64 | bdb9b1d8b3feb5fb |
| interp_intrinsic_hints | 61d53957771893ce | ok | ok | ok | n/a: no Prover.toml | () | 9d7a0a6cd38cc0e2 |
| interp_match_enum | fe8d3d68db554db0 | ok | ok | ok | n/a: no recorded return | 12u32 | 060efb68df11e37a |
| interp_match_int | e1511e0ce0ce5950 | ok | ok | ok | n/a: no recorded return | 300i32 | 5b1868ebc642598d |
| interp_reached_dep_error | b8e0a7cdc63bb13e | ok | FAIL DependencyCompileGap: program reaches dependency code from files that failed elaboration under the chos…#d581b3ff | n/a: not compiled | n/a: not interpreted |  |  |
| interp_refs_call_chain | 864912e8a975853e | ok | ok | ok | n/a: no recorded return | 110u64 | 72f9caa1a735912d |
| interp_refs_double_deref_alias | 0550fa07fa91ea5e | ok | ok | ok | n/a: no Prover.toml | () | 510911ca84c5c372 |
| interp_refs_nested_field | 58f4ab8212edd0c3 | ok | ok | ok | n/a: no Prover.toml | () | 70dacc5961f737da |
| interp_refs_struct_field | 203da9ac64b916da | ok | ok | ok | n/a: no Prover.toml | () | 82e34e798f198512 |
| intrinsic_conversions | a4c3c58bc1f41159 | ok | ok | ok | n/a: no Prover.toml | () | 1bb83447a0ae27e4 |
| intrinsic_range_constraint | cd601ed04451b076 | ok | FAIL DependencyCompileGap: program reaches dependency code from files that failed elaboration under the chos…#90ec0f4a | n/a: not compiled | n/a: not interpreted |  |  |
| intrinsic_slice_ops | 29b9e9272e434132 | ok | ok | ok | n/a: no Prover.toml | () | caef701a903c50ae |
| intrinsic_to_bytes | bc304c9c1b2f5d32 | ok | FAIL DependencyCompileGap: program reaches dependency code from files that failed elaboration under the chos…#90ec0f4a | n/a: not compiled | n/a: not interpreted |  |  |
| neg_assert_fail | c4a48169683ef0f9 | ok | ok | FAIL AssertionFailed | n/a: not interpreted |  | b2cf61366f39f8e5 |
| neg_assert_fmt_msg | c5a94309a8d20f42 | ok | ok | FAIL AssertionFailed: sum=45 field=0x03 array=[1, 2] tuple=(7,) point=Point { x: 4, y: true } choice=Choice::Some(9) | n/a: not interpreted |  | 5d8fdc1b4dbffae7 |
| neg_interp_inputs_i64 | 6251dffad749bb13 | ok | ok | FAIL InputError: failed to parse Prover.toml: The value passed for parameter `x` is invalid: Value -1 exceed…#c4ab7790 | n/a: not interpreted |  | f3402f7b81a0fb69 |
| neg_reachable_error | 98b1cd88e2c07d22 | ok | FAIL CompileError: Expected type bool, found type u64 | n/a: not compiled | n/a: not interpreted |  |  |

## Totals

| step | ok | failed | not run |
| --- | --- | --- | --- |
| load | 506 | 0 | 3 |
| compile | 438 | 68 | 3 |
| interpret | 351 | 87 | 71 |
| oracle | 145 | 3 | 361 |
| projection | 438 | 0 | 71 |

### Corpus failures by step and kind

| step: kind | count |
| --- | --- |
| compile: CompileError | 15 |
| compile: DependencyCompileGap | 37 |
| compile: Panic | 16 |
| interpret: AssertionFailed | 1 |
| interpret: InputError | 8 |
| interpret: Unsupported(array_as_str_unchecked on non-UTF-8 bytes) | 1 |
| interpret: Unsupported(break/continue in value position) | 2 |
| interpret: Unsupported(dereference of a non-reference value in lvalue) | 3 |
| interpret: Unsupported(dereference of a non-reference value) | 3 |
| interpret: Unsupported(format string interpolating a field) | 3 |
| interpret: Unsupported(intrinsic) | 16 |
| interpret: Unsupported(non-UTF-8 string literal) | 1 |
| interpret: Unsupported(oracle call) | 45 |
| interpret: Unsupported(signed 64-bit ABI input is not representable in this field) | 4 |
| oracle: OracleMismatch | 1 |
| oracle: Unsupported(signed 64-bit ABI input is not representable in this field) | 2 |
