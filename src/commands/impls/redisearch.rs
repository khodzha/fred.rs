use crate::{
  commands::{args_values_cmd, one_arg_values_cmd, COUNT, LEN, LIMIT, WITHCODE},
  error::RedisError,
  interfaces::ClientLike,
  protocol::{command::RedisCommandKind, utils as protocol_utils},
  types::{
    AggregateOperation,
    FtAggregateOptions,
    FtAlterOptions,
    FtCreateOptions,
    FtSearchOptions,
    Load,
    MultipleStrings,
    RedisKey,
    RedisValue,
    SearchSchema,
    SpellcheckTerms,
  },
  utils,
};
use bytes::Bytes;
use bytes_utils::Str;

static DD: &str = "DD";
static DIALECT: &str = "DIALECT";
static DISTANCE: &str = "DISTANCE";
static INCLUDE: &str = "INCLUDE";
static EXCLUDE: &str = "EXCLUDE";
static TERMS: &str = "TERMS";
static INCR: &str = "INCR";
static PAYLOAD: &str = "PAYLOAD";
static FUZZY: &str = "FUZZY";
static WITHSCORES: &str = "WITHSCORES";
static WITHPAYLOADS: &str = "WITHPAYLOADS";
static MAX: &str = "MAX";
static SKIPINITIALSCAN: &str = "SKIPINITIALSCAN";
static NOCONTENT: &str = "NOCONTENT";
static VERBATIM: &str = "VERBATIM";
static NOSTOPWORDS: &str = "NOSTOPWORDS";
static WITHSORTKEYS: &str = "WITHSORTKEYS";
static FILTER: &str = "FILTER";
static GEOFILTER: &str = "GEOFILTER";
static INKEYS: &str = "INKEYS";
static INFIELDS: &str = "INFIELDS";
static _RETURN: &str = "RETURN";
static AS: &str = "AS";
static SUMMARIZE: &str = "SUMMARIZE";
static FIELDS: &str = "FIELDS";
static FRAGS: &str = "FRAGS";
static SEPARATOR: &str = "SEPARATOR";
static HIGHLIGHT: &str = "HIGHLIGHT";
static TAGS: &str = "TAGS";
static SLOP: &str = "SLOP";
static TIMEOUT: &str = "TIMEOUT";
static INORDER: &str = "INORDER";
static LANGUAGE: &str = "LANGUAGE";
static EXPANDER: &str = "EXPANDER";
static SCORER: &str = "SCORER";
static EXPLAINSCORE: &str = "EXPLAINSCORE";
static SORTBY: &str = "SORTBY";
static PARAMS: &str = "PARAMS";
static WITHCOUNT: &str = "WITHCOUNT";
static LOAD: &str = "LOAD";
static WITHCURSOR: &str = "WITHCURSOR";
static MAXIDLE: &str = "MAXIDLE";
static APPLY: &str = "APPLY";
static GROUPBY: &str = "GROUPBY";
static REDUCE: &str = "REDUCE";

fn gen_aggregate_op(args: &mut Vec<RedisValue>, operation: AggregateOperation) -> Result<(), RedisError> {
  match operation {
    AggregateOperation::Filter { expression } => {
      args.extend([static_val!(FILTER), expression.into()]);
    },
    AggregateOperation::Limit { offset, num } => {
      args.extend([static_val!(LIMIT), offset.try_into()?, num.try_into()?]);
    },
    AggregateOperation::Apply { expression, name } => {
      args.extend([static_val!(APPLY), expression.into(), static_val!(AS), name.into()]);
    },
    AggregateOperation::SortBy { properties, max } => {
      args.extend([static_val!(SORTBY), properties.len().try_into()?]);
      for (property, order) in properties.into_iter() {
        args.extend([property.into(), order.to_str().into()]);
      }
      if let Some(max) = max {
        args.extend([static_val!(MAX), max.try_into()?]);
      }
    },
    AggregateOperation::GroupBy { fields, reducers } => {
      args.extend([static_val!(GROUPBY), fields.len().try_into()?]);
      args.extend(fields.into_iter().map(|f| f.into()));

      for reducer in reducers.into_iter() {
        args.extend([
          static_val!(REDUCE),
          static_val!(reducer.func.to_str()),
          reducer.args.len().try_into()?,
        ]);
        args.extend(reducer.args.into_iter().map(|a| a.into()));
        if let Some(name) = reducer.name {
          args.extend([static_val!(AS), name.into()]);
        }
      }
    },
  };

  Ok(())
}

fn gen_aggregate_options(args: &mut Vec<RedisValue>, options: FtAggregateOptions) -> Result<(), RedisError> {
  if options.verbatim {
    args.push(static_val!(VERBATIM));
  }
  if let Some(load) = options.load {
    match load {
      Load::All => {
        args.push(static_val!(LOAD));
        args.push(static_val!("*"));
      },
      Load::Some(fields) => {
        if !fields.is_empty() {
          args.push(static_val!(LOAD));
          args.push(fields.len().try_into()?);
          for field in fields.into_iter() {
            args.push(field.identifier.into());
            if let Some(property) = field.property {
              args.extend([static_val!(AS), property.into()]);
            }
          }
        }
      },
    }
  }
  if let Some(timeout) = options.timeout {
    args.extend([static_val!(TIMEOUT), timeout.into()]);
  }
  for operation in options.pipeline.into_iter() {
    gen_aggregate_op(args, operation)?;
  }
  if let Some(cursor) = options.cursor {
    args.push(static_val!(WITHCURSOR));
    if let Some(count) = cursor.count {
      args.extend([static_val!(COUNT), count.try_into()?]);
    }
    if let Some(idle) = cursor.max_idle {
      args.extend([static_val!(MAXIDLE), idle.try_into()?]);
    }
  }
  if !options.params.is_empty() {
    args.extend([static_val!(PARAMS), options.params.len().try_into()?]);
    for param in options.params.into_iter() {
      args.extend([param.name.into(), param.value.into()]);
    }
  }
  if let Some(dialect) = options.dialect {
    args.extend([static_val!(DIALECT), dialect.into()]);
  }

  Ok(())
}

fn gen_search_options(args: &mut Vec<RedisValue>, options: FtSearchOptions) -> Result<(), RedisError> {
  if options.nocontent {
    args.push(static_val!(NOCONTENT));
  }
  if options.verbatim {
    args.push(static_val!(VERBATIM));
  }
  if options.nostopwords {
    args.push(static_val!(NOSTOPWORDS));
  }
  if options.withscores {
    args.push(static_val!(WITHSCORES));
  }
  if options.withpayloads {
    args.push(static_val!(WITHPAYLOADS));
  }
  if options.withsortkeys {
    args.push(static_val!(WITHSORTKEYS));
  }
  for filter in options.filters.into_iter() {
    args.extend([
      static_val!(FILTER),
      filter.attribute.into(),
      filter.min.into_value()?,
      filter.max.into_value()?,
    ]);
  }
  for geo_filter in options.geofilters.into_iter() {
    args.extend([
      static_val!(GEOFILTER),
      geo_filter.attribute.into(),
      geo_filter.position.longitude.try_into()?,
      geo_filter.position.latitude.try_into()?,
      geo_filter.radius,
      geo_filter.units.to_str().into(),
    ]);
  }
  if !options.inkeys.is_empty() {
    args.push(static_val!(INKEYS));
    args.push(options.inkeys.len().try_into()?);
    args.extend(options.inkeys.into_iter().map(|k| k.into()));
  }
  if !options.infields.is_empty() {
    args.push(static_val!(INFIELDS));
    args.push(options.infields.len().try_into()?);
    args.extend(options.infields.into_iter().map(|s| s.into()));
  }
  if !options.r#return.is_empty() {
    args.extend([static_val!(_RETURN), options.r#return.len().try_into()?]);
    for field in options.r#return.into_iter() {
      args.push(field.identifier.into());
      if let Some(property) = field.property {
        args.push(static_val!(AS));
        args.push(property.into());
      }
    }
  }
  if let Some(summarize) = options.summarize {
    args.push(static_val!(SUMMARIZE));
    if !summarize.fields.is_empty() {
      args.push(static_val!(FIELDS));
      args.push(summarize.fields.len().try_into()?);
      args.extend(summarize.fields.into_iter().map(|s| s.into()));
    }
    if let Some(frags) = summarize.frags {
      args.push(static_val!(FRAGS));
      args.push(frags.try_into()?);
    }
    if let Some(len) = summarize.len {
      args.push(static_val!(LEN));
      args.push(len.try_into()?);
    }
    if let Some(separator) = summarize.separator {
      args.push(static_val!(SEPARATOR));
      args.push(separator.into());
    }
  }
  if let Some(highlight) = options.highlight {
    args.push(static_val!(HIGHLIGHT));
    if !highlight.fields.is_empty() {
      args.push(static_val!(FIELDS));
      args.push(highlight.fields.len().try_into()?);
      args.extend(highlight.fields.into_iter().map(|s| s.into()));
    }
    if let Some((open, close)) = highlight.tags {
      args.extend([static_val!(TAGS), open.into(), close.into()]);
    }
  }
  if let Some(slop) = options.slop {
    args.extend([static_val!(SLOP), slop.into()]);
  }
  if let Some(timeout) = options.timeout {
    args.extend([static_val!(TIMEOUT), timeout.into()]);
  }
  if options.inorder {
    args.push(static_val!(INORDER));
  }
  if let Some(language) = options.language {
    args.extend([static_val!(LANGUAGE), language.into()]);
  }
  if let Some(expander) = options.expander {
    args.extend([static_val!(EXPANDER), expander.into()]);
  }
  if let Some(scorer) = options.scorer {
    args.extend([static_val!(SCORER), scorer.into()]);
  }
  if options.explainscore {
    args.push(static_val!(EXPLAINSCORE));
  }
  if let Some(payload) = options.payload {
    args.extend([static_val!(PAYLOAD), RedisValue::Bytes(payload)]);
  }
  if let Some(sort) = options.sortby {
    args.push(static_val!(SORTBY));
    args.push(sort.attribute.into());
    if let Some(order) = sort.order {
      args.push(order.to_str().into());
    }
    if sort.withcount {
      args.push(static_val!(WITHCOUNT));
    }
  }
  if let Some((offset, count)) = options.limit {
    args.extend([static_val!(LIMIT), offset.into(), count.into()]);
  }
  if !options.params.is_empty() {
    args.push(static_val!(PARAMS));
    args.push(options.params.len().try_into()?);
    for param in options.params.into_iter() {
      args.extend([param.name.into(), param.value.into()]);
    }
  }
  if let Some(dialect) = options.dialect {
    args.extend([static_val!(DIALECT), dialect.into()]);
  }

  Ok(())
}

fn gen_alter_options(args: &mut Vec<RedisValue>, options: FtAlterOptions) -> Result<(), RedisError> {
  unimplemented!()
}

fn gen_create_options(args: &mut Vec<RedisValue>, options: FtCreateOptions) -> Result<(), RedisError> {
  unimplemented!()
}

fn gen_schema_args(args: &mut Vec<RedisValue>, options: SearchSchema) -> Result<(), RedisError> {
  unimplemented!()
}

pub async fn ft_list<C: ClientLike>(client: &C) -> Result<RedisValue, RedisError> {
  args_values_cmd(client, RedisCommandKind::FtList, vec![]).await
}

pub async fn ft_aggregate<C: ClientLike>(
  client: &C,
  index: Str,
  query: Str,
  options: FtAggregateOptions,
) -> Result<RedisValue, RedisError> {
  let frame = utils::request_response(client, move || {
    let mut args = Vec::with_capacity(2 + options.num_args());
    args.push(index.into());
    args.push(query.into());
    gen_aggregate_options(&mut args, options)?;

    Ok((RedisCommandKind::FtAggregate, args))
  })
  .await?;

  protocol_utils::frame_to_results(frame)
}

pub async fn ft_search<C: ClientLike>(
  client: &C,
  index: Str,
  query: Str,
  options: FtSearchOptions,
) -> Result<RedisValue, RedisError> {
  let frame = utils::request_response(client, move || {
    let mut args = Vec::with_capacity(2 + options.num_args());
    args.push(index.into());
    args.push(query.into());
    gen_search_options(&mut args, options)?;

    Ok((RedisCommandKind::FtSearch, args))
  })
  .await?;

  protocol_utils::frame_to_results(frame)
}

pub async fn ft_create<C: ClientLike>(
  client: &C,
  index: Str,
  options: FtCreateOptions,
  schema: Vec<SearchSchema>,
) -> Result<RedisValue, RedisError> {
  let frame = utils::request_response(client, move || {
    let schema_num_args = schema.iter().fold(0, |m, s| m + s.num_args());
    let mut args = Vec::with_capacity(2 + options.num_args() + schema_num_args);
    args.push(index.into());
    gen_create_options(&mut args, options)?;

    for schema in schema.into_iter() {
      gen_schema_args(&mut args, schema)?;
    }

    Ok((RedisCommandKind::FtCreate, args))
  })
  .await?;

  protocol_utils::frame_to_results(frame)
}

pub async fn ft_alter<C: ClientLike>(
  client: &C,
  index: Str,
  options: FtAlterOptions,
) -> Result<RedisValue, RedisError> {
  let frame = utils::request_response(client, move || {
    let mut args = Vec::with_capacity(1 + options.num_args());
    args.push(index.into());
    gen_alter_options(&mut args, options)?;

    Ok((RedisCommandKind::FtAlter, args))
  })
  .await?;

  protocol_utils::frame_to_results(frame)
}

pub async fn ft_aliasadd<C: ClientLike>(client: &C, alias: Str, index: Str) -> Result<RedisValue, RedisError> {
  args_values_cmd(client, RedisCommandKind::FtAliasAdd, vec![alias.into(), index.into()]).await
}

pub async fn ft_aliasdel<C: ClientLike>(client: &C, alias: Str) -> Result<RedisValue, RedisError> {
  args_values_cmd(client, RedisCommandKind::FtAliasDel, vec![alias.into()]).await
}

pub async fn ft_aliasupdate<C: ClientLike>(client: &C, alias: Str, index: Str) -> Result<RedisValue, RedisError> {
  args_values_cmd(client, RedisCommandKind::FtAliasUpdate, vec![
    alias.into(),
    index.into(),
  ])
  .await
}

pub async fn ft_config_get<C: ClientLike>(client: &C, option: Str) -> Result<RedisValue, RedisError> {
  args_values_cmd(client, RedisCommandKind::FtConfigGet, vec![option.into()]).await
}

pub async fn ft_config_set<C: ClientLike>(
  client: &C,
  option: Str,
  value: RedisValue,
) -> Result<RedisValue, RedisError> {
  args_values_cmd(client, RedisCommandKind::FtConfigSet, vec![option.into(), value]).await
}

pub async fn ft_cursor_del<C: ClientLike>(
  client: &C,
  index: Str,
  cursor: RedisValue,
) -> Result<RedisValue, RedisError> {
  args_values_cmd(client, RedisCommandKind::FtCursorDel, vec![index.into(), cursor]).await
}

pub async fn ft_cursor_read<C: ClientLike>(
  client: &C,
  index: Str,
  cursor: RedisValue,
  count: Option<u64>,
) -> Result<RedisValue, RedisError> {
  let args = if let Some(count) = count {
    vec![index.into(), cursor, static_val!(COUNT), count.try_into()?]
  } else {
    vec![index.into(), cursor]
  };

  args_values_cmd(client, RedisCommandKind::FtCursorRead, args).await
}

pub async fn ft_dictadd<C: ClientLike>(
  client: &C,
  dict: Str,
  terms: MultipleStrings,
) -> Result<RedisValue, RedisError> {
  let frame = utils::request_response(client, move || {
    let mut args = Vec::with_capacity(terms.len() + 1);
    args.push(dict.into());
    for term in terms.inner().into_iter() {
      args.push(term.into());
    }

    Ok((RedisCommandKind::FtDictAdd, args))
  })
  .await?;

  protocol_utils::frame_to_results(frame)
}

pub async fn ft_dictdel<C: ClientLike>(
  client: &C,
  dict: Str,
  terms: MultipleStrings,
) -> Result<RedisValue, RedisError> {
  let frame = utils::request_response(client, move || {
    let mut args = Vec::with_capacity(terms.len() + 1);
    args.push(dict.into());
    for term in terms.inner().into_iter() {
      args.push(term.into());
    }

    Ok((RedisCommandKind::FtDictDel, args))
  })
  .await?;

  protocol_utils::frame_to_results(frame)
}

pub async fn ft_dictdump<C: ClientLike>(client: &C, dict: Str) -> Result<RedisValue, RedisError> {
  one_arg_values_cmd(client, RedisCommandKind::FtDictDump, dict.into()).await
}

pub async fn ft_dropindex<C: ClientLike>(client: &C, index: Str, dd: bool) -> Result<RedisValue, RedisError> {
  let args = if dd {
    vec![index.into(), static_val!(DD)]
  } else {
    vec![index.into()]
  };

  args_values_cmd(client, RedisCommandKind::FtDropIndex, args).await
}

pub async fn ft_explain<C: ClientLike>(
  client: &C,
  index: Str,
  query: Str,
  dialect: Option<i64>,
) -> Result<RedisValue, RedisError> {
  let args = if let Some(dialect) = dialect {
    vec![index.into(), query.into(), static_val!(DIALECT), dialect.into()]
  } else {
    vec![index.into(), query.into()]
  };

  args_values_cmd(client, RedisCommandKind::FtExplain, args).await
}

pub async fn ft_info<C: ClientLike>(client: &C, index: Str) -> Result<RedisValue, RedisError> {
  one_arg_values_cmd(client, RedisCommandKind::FtInfo, index.into()).await
}

pub async fn ft_spellcheck<C: ClientLike>(
  client: &C,
  index: Str,
  query: Str,
  distance: Option<u8>,
  terms: Option<SpellcheckTerms>,
  dialect: Option<i64>,
) -> Result<RedisValue, RedisError> {
  let frame = utils::request_response(client, move || {
    let terms_len = terms.as_ref().map(|t| t.num_args()).unwrap_or(0);
    let mut args = Vec::with_capacity(9 + terms_len);
    args.push(index.into());
    args.push(query.into());

    if let Some(distance) = distance {
      args.push(static_val!(DISTANCE));
      args.push(distance.into());
    }
    if let Some(terms) = terms {
      args.push(static_val!(TERMS));
      let (dictionary, terms) = match terms {
        SpellcheckTerms::Include { dictionary, terms } => {
          args.push(static_val!(INCLUDE));
          (dictionary, terms)
        },
        SpellcheckTerms::Exclude { dictionary, terms } => {
          args.push(static_val!(EXCLUDE));
          (dictionary, terms)
        },
      };

      args.push(dictionary.into());
      for term in terms.into_iter() {
        args.push(term.into());
      }
    }
    if let Some(dialect) = dialect {
      args.push(static_val!(DIALECT));
      args.push(dialect.into());
    }

    Ok((RedisCommandKind::FtSpellCheck, args))
  })
  .await?;

  protocol_utils::frame_to_results(frame)
}

pub async fn ft_sugadd<C: ClientLike>(
  client: &C,
  key: RedisKey,
  string: Str,
  score: f64,
  incr: bool,
  payload: Option<Bytes>,
) -> Result<RedisValue, RedisError> {
  let frame = utils::request_response(client, move || {
    let mut args = Vec::with_capacity(6);
    args.push(key.into());
    args.push(string.into());
    args.push(score.try_into()?);

    if incr {
      args.push(static_val!(INCR));
    }
    if let Some(payload) = payload {
      args.push(static_val!(PAYLOAD));
      args.push(RedisValue::Bytes(payload));
    }

    Ok((RedisCommandKind::FtSugAdd, args))
  })
  .await?;

  protocol_utils::frame_to_results(frame)
}

pub async fn ft_sugdel<C: ClientLike>(client: &C, key: RedisKey, string: Str) -> Result<RedisValue, RedisError> {
  args_values_cmd(client, RedisCommandKind::FtSugDel, vec![key.into(), string.into()]).await
}

pub async fn ft_sugget<C: ClientLike>(
  client: &C,
  key: RedisKey,
  prefix: Str,
  fuzzy: bool,
  withscores: bool,
  withpayloads: bool,
  max: Option<u64>,
) -> Result<RedisValue, RedisError> {
  let frame = utils::request_response(client, move || {
    let mut args = Vec::with_capacity(7);
    args.push(key.into());
    args.push(prefix.into());
    if fuzzy {
      args.push(static_val!(FUZZY));
    }
    if withscores {
      args.push(static_val!(WITHSCORES));
    }
    if withpayloads {
      args.push(static_val!(WITHPAYLOADS));
    }
    if let Some(max) = max {
      args.push(static_val!(MAX));
      args.push(max.try_into()?);
    }

    Ok((RedisCommandKind::FtSugGet, args))
  })
  .await?;

  protocol_utils::frame_to_results(frame)
}

pub async fn ft_suglen<C: ClientLike>(client: &C, key: RedisKey) -> Result<RedisValue, RedisError> {
  one_arg_values_cmd(client, RedisCommandKind::FtSugLen, key.into()).await
}

pub async fn ft_syndump<C: ClientLike>(client: &C, index: Str) -> Result<RedisValue, RedisError> {
  one_arg_values_cmd(client, RedisCommandKind::FtSynDump, index.into()).await
}

pub async fn ft_synupdate<C: ClientLike>(
  client: &C,
  index: Str,
  synonym_group_id: Str,
  skipinitialscan: bool,
  terms: MultipleStrings,
) -> Result<RedisValue, RedisError> {
  let frame = utils::request_response(client, move || {
    let mut args = Vec::with_capacity(3 + terms.len());
    args.push(index.into());
    args.push(synonym_group_id.into());
    if skipinitialscan {
      args.push(static_val!(SKIPINITIALSCAN));
    }
    for term in terms.inner().into_iter() {
      args.push(term.into());
    }

    Ok((RedisCommandKind::FtSynUpdate, args))
  })
  .await?;

  protocol_utils::frame_to_results(frame)
}

pub async fn ft_tagvals<C: ClientLike>(client: &C, index: Str, field_name: Str) -> Result<RedisValue, RedisError> {
  args_values_cmd(client, RedisCommandKind::FtTagVals, vec![
    index.into(),
    field_name.into(),
  ])
  .await
}
