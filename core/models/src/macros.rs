/// A macro for creating SignedPods with reduced boilerplate.
///
/// This macro simplifies the creation of signed pods by automatically handling
/// the builder pattern and signing process.
///
/// # Syntax
/// use podnet_models::signed_pod;
///
/// signed_pod!(pod_params, secret_key, {
///     "field1" => value1,
///     "field2" => value2,
///     // ... more fields
/// })
///
/// # Arguments
/// * `pod_params` - The pod parameters from `PodNetProverSetup::get_params()`
/// * `secret_key` - The secret key for signing (will be cloned automatically)
/// * Fields - Key-value pairs where keys are strings and values are POD-compatible types
///
/// # Returns
/// Returns a `Result<SignedPod, Error>` that must be handled with `?` or `.unwrap()`.
///
/// # Example
/// use podnet_models::signed_pod;
///
/// let params = PodNetProverSetup::get_params();
/// let secret_key = /* your secret key */;
///
/// let pod = signed_pod!(&params, secret_key.clone(), {
///     "request_type" => "publish",
///     "content_hash" => content_hash,
///     "post_id" => post_id_num.unwrap_or(-1),
///     "tags" => tag_set,
/// })?;
#[macro_export]
macro_rules! signed_pod {
    ($params:expr, $secret_key:expr, {
        $($key:expr => $value:expr),* $(,)?
    }) => {{
        let mut builder = SignedPodBuilder::new($params);
        $(
            builder.insert($key, $value);
        )*
        builder.sign(&mut Signer($secret_key))?
    }};
}
