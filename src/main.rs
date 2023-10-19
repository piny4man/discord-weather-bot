use anyhow::anyhow;
use serenity::model::application::interaction::{Interaction, InteractionResponseType};
use serenity::model::gateway::Ready;
use serenity::model::prelude::command::CommandOptionType;
use serenity::prelude::*;
use serenity::{async_trait, model::prelude::GuildId};
use shuttle_secrets::SecretStore;
use tracing::{error, info};

mod weather;

struct Bot {
    weather_api_key: String,
    client: reqwest::Client,
    discord_guild_id: GuildId,
}

#[async_trait]
impl EventHandler for Bot {
    async fn ready(&self, ctx: Context, ready: Ready) {
        info!("{} is connected!", ready.user.name);

        let commands =
            GuildId::set_application_commands(&self.discord_guild_id, &ctx.http, |commands| {
                commands
                    .create_application_command(|command| {
                        command.name("hello").description("Que pasa pinyas!")
                    })
                    .create_application_command(|command| {
                        command
                            .name("weather")
                            .description("Display the weather")
                            .create_option(|option| {
                                option
                                    .name("place")
                                    .description("City to lookup forecast")
                                    .kind(CommandOptionType::String)
                                    .required(true)
                            })
                    })
            })
            .await
            .unwrap();

        info!("Registered commands: {:?}", commands);
    }

    async fn interaction_create(&self, ctx: Context, interaction: Interaction) {
        if let Interaction::ApplicationCommand(command) = interaction {
            let response_content = match command.data.name.as_str() {
                "hello" => "Que pasa pinyas".to_owned(),
                "weater" => {
                    let argument = command
                        .data
                        .options
                        .iter()
                        .find(|opt| opt.name == "place")
                        .cloned();

                    let value = argument.unwrap().value.unwrap();
                    let place = value.as_str().unwrap();
                    let result = weather::get_forecast(place, api_key, client).await;

                    match result {
                        Ok((location, forecast)) => {
                            format!("Forecast: {} in {}", forecast.headline.overview, location)
                        }
                        Err(err) => {
                            format!("Err: {}", err)
                        }
                    }
                }
                command => unreachable!("Unknown command: {}", command),
            };
            let create_interaction_response =
                command.create_interaction_response(&ctx.http, |response| {
                    response
                        .kind(InteractionResponseType::ChannelMessageWithSource)
                        .interaction_response_data(|message| message.content(response_content))
                });

            if let Err(why) = create_interaction_response.await {
                eprint!("Cannot respond to slash command: {:?}", why)
            }
        }
    }
}

#[shuttle_runtime::main]
async fn serenity(
    #[shuttle_secrets::Secrets] secret_store: SecretStore,
) -> shuttle_serenity::ShuttleSerenity {
    // Get the discord token set in `Secrets.toml`
    let token = if let Some(token) = secret_store.get("DISCORD_TOKEN") {
        token
    } else {
        return Err(anyhow!("'DISCORD_TOKEN' was not found").into());
    };

    let weather_api_key = if let Some(weather_api_key) = secret_store.get("WEATHER_API_KEY") {
        weather_api_key
    } else {
        return Err(anyhow!("'WEATHER_API_KEY' was not found").into());
    };

    let discord_guild_id = if let Some(weather_api_key) = secret_store.get("DISCORD_GUILD_ID") {
        weather_api_key
    } else {
        return Err(anyhow!("'DISCORD_GUILD_ID' was not found").into());
    };

    // Set gateway intents, which decides what events the bot will be notified about
    let intents = GatewayIntents::GUILD_MESSAGES | GatewayIntents::MESSAGE_CONTENT;

    let client = Client::builder(&token, intents)
        .event_handler(Bot {
            weather_api_key,
            client: reqwest::Client::new(),
            discord_guild_id: GuildId(discord_guild_id.parse().unwrap()),
        })
        .await
        .expect("Err creating client");

    Ok(client.into())
}
