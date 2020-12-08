use crate::hdk3::prelude::*;

#[hdk_entry(id = "post", required_validations = 5)]
struct Post(String);

#[hdk_entry(id = "msg", required_validations = 5)]
struct Msg(String);

entry_defs![Post::entry_def(), Msg::entry_def()];

fn post() -> Post {
    Post("foo".into())
}

fn msg() -> Msg {
    Msg("hi".into())
}

#[hdk_extern]
fn create_entry(_: ()) -> ExternResult<HeaderHash> {
    Ok(crate::hdk3::prelude::create_entry(&post())?)
}

#[hdk_extern]
fn get_entry(_: ()) -> ExternResult<GetOutput> {
    Ok(GetOutput::new(get(hash_entry(&post())?, GetOptions)?))
}

#[hdk_extern]
fn update_entry(_: ()) -> ExternResult<HeaderHash> {
    let header_hash = crate::hdk3::prelude::create_entry(&post())?;
    Ok(crate::hdk3::prelude::update_entry(header_hash, &post())?)
}

#[hdk_extern]
/// Updates to a different entry, this will fail
fn invalid_update_entry(_: ()) -> ExternResult<HeaderHash> {
    let header_hash = crate::hdk3::prelude::create_entry(&post())?;
    Ok(crate::hdk3::prelude::update_entry(header_hash, &msg())?)
}
