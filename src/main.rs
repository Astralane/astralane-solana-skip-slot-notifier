use std::env;
use serenity::http::Http;
use serenity::model::webhook::Webhook;

use serde::{Serialize,Deserialize};
use reqwest;
use std::{thread, time::Duration};
use jito_geyser_protos::solana::geyser::{geyser_client::GeyserClient,SubscribeSlotUpdateRequest};
use tonic::transport::Endpoint;
use tonic::{Status};
use std::str::FromStr;

#[derive(Serialize)]
struct BlockProduction{
    jsonrpc: String,
    id: i32,
    method: String
}
#[derive(Deserialize,Debug)]
struct BlockInfo{
    result: EpochValue,
}
#[derive(Deserialize,Debug)]
struct EpochValue{
    value: EpochInfo
}
#[derive(Deserialize,Debug)]
struct EpochInfo{
    range:EpochRange
}
#[derive(Deserialize,Debug)]
struct EpochRange{
    firstSlot: u64,
    lastSlot: u64
}
#[derive(Deserialize,Debug)]
struct LsResult{
    result: Ls,
}
#[derive(Deserialize,Debug)]
struct Ls{
    DevBQDYfHJSnnmA7kkeFgE9dekK1gaGazynZcjgxA577: Option<Vec<u64>> // <------------------------------------ change this to yours
}
#[derive(Serialize)]
struct LBP{
    jsonrpc: String,
    id: i32,
    method: String,
    params:[LBI;1]    
}
#[derive(Serialize)]
struct LBI {
    identity: String
}
#[tokio::main]
async fn main() {
    println!("starting slot bot");
    let token= env::var("BOT_TOKEN").expect("missing env ");
    let grpc_url=env::var("GRPC_URL").expect("missing env GRPC_URL");
    let api_endpoint= env::var("API").expect("missing api endpoint");
    // You don't need a token when you are only dealing with webhooks.
    let http = Http::new("");
    let webhook = Webhook::from_url(&http, &token.to_string())
        .await
        .unwrap();
    let mut prev_first_slot:u64 =0;
    loop {
        
        let (first_slot,last_slot)=fetch_boundaries(&api_endpoint).await;
        if prev_first_slot!=first_slot{
            let msg=format!("first and last slot for the epoch is {first_slot} {last_slot}");
            webhook.execute(&http, false, |w| w.content(msg)).await.expect("Could not execute webhook.");
            let leader_slots: Vec<u64> = Vec::new();
            let leader_slots=fetch_leader(&api_endpoint,leader_slots,&first_slot).await;
            let (completed_slots,skipped_slots,unknown_slots,total_slots) = slot_stream(leader_slots,&grpc_url,webhook.clone()).await;
            let msg=format!("completed ={completed_slots}, skipped ={skipped_slots}, unknown = {unknown_slots}, total = {total_slots}");
            webhook.execute(&http, false, |w| w.content(msg)).await.expect("Could not execute webhook."); 
            prev_first_slot=first_slot;             
        }else{
            thread::sleep(Duration::from_millis(5000));
        }
         
    }
        

}
// 2) get leader schedule for that epoch and get total number of slots
async fn fetch_boundaries(api: &String)-> (u64,u64){
    let http_api=format!("https://{}",api);
    let bp=BlockProduction{
        jsonrpc:"2.0".to_string(),
        id:1,
        method:"getBlockProduction".to_string()
    };
    //let req=serde_json::to_string(&bp).expect("couldn't convert json to string");
    let client=reqwest::Client::new();
    let res=client.post(http_api).json(&bp).send().await.expect("cannot send post req");
    let res: BlockInfo=res.json().await.expect("cannot parse");
    (res.result.value.range.firstSlot,res.result.value.range.lastSlot)

}

// 1) create a function to fetch the last slot of the epoch
async fn fetch_leader(api: &String,mut ls: Vec<u64>,base: &u64) -> Vec<u64>{
    let body = LBP{
        jsonrpc:"2.0".to_string(),
        id:1,
        method:"getLeaderSchedule".to_string(),
        params:[LBI{identity:"DevBQDYfHJSnnmA7kkeFgE9dekK1gaGazynZcjgxA577".to_string()}] //  <--------------------- change this to yours     
    }; //to lazy to make structs out of this 
    let http_api=format!("https://{}",api);
    let client=reqwest::Client::new();
    let res=client.post(http_api).json(&body).send().await.expect("cannot send post req");
    let res: LsResult=res.json().await.expect("cannot parse leader");
    match res.result.DevBQDYfHJSnnmA7kkeFgE9dekK1gaGazynZcjgxA577 { // <----------------------------- change this to yours
        Some(T) => {
            for i in T.iter(){
                let final_slot=i+base;
                ls.push(final_slot)
            }
        }
        None => println!("No slots")
    };
    ls
}


//3) run a seperate thread for the wss and check a. we do not cross the last slot if so break b. if we sub to our slot then return completed else send a discod message

async fn slot_stream(mut leader_slots: Vec<u64>, api: &String, webhook: Webhook) -> (i32,i32,i32,i32) {

    let http = Http::new("");
    let grpc_api=format!("http://{}",api);
    let endpoint = Endpoint::from_str(&grpc_api).unwrap();
    println!("connected to grpc");
    let channel = endpoint.connect().await.expect("cannot connect to channel");
    let mut client = GeyserClient::with_interceptor(channel, intrcptr);
    let mut stream = client.subscribe_slot_updates(SubscribeSlotUpdateRequest {}).await.expect("couldn't get stream").into_inner();

    let mut current_slot: u64 = 0;
    let mut completed_slots = 0;
    let mut skipped_slots = 0;
    let mut unknown_slots =0;
    let total_slots=leader_slots.len();
    let last_slot=leader_slots[leader_slots.len()-1]+32;//buffer incase you skip the last slot d
    while current_slot < last_slot{// 0
        let slot_grpc=stream.message().await;
        match slot_grpc{
            Ok(m) => {
                let slot_up=m.unwrap().slot_update.expect("cannot unwrap slot_update");
                if slot_up.status !=0{
                    continue    ;
                }
                println!("{slot_up:?}");
                current_slot=slot_up.slot;//chnaged from slots to parent for addded commitment
                let mut len_vec: usize=leader_slots.len();
                let mut index =0;
                while index < len_vec{// TODO restructure so that you first remove unknown then do evenrthing without a loop

                   
                    let ls=leader_slots[index];//92
                    if current_slot > ls  { //don't know what happend could have started the code half way through the epoch
                        let slot_diff=current_slot-ls;//1
                        if slot_diff > 20{
                            unknown_slots+=1;
                            leader_slots.remove(index);                    
                            println!("unknown slots{leader_slots:?}");
                        }else if slot_diff > 4{
                            skipped_slots+=1;
                            let msg=format!("skipped slot {} total skip= {skipped_slots}",leader_slots[0]);
                            leader_slots.remove(index);
                            webhook.execute(&http, false, |w| w.content(msg)).await.expect("Could not execute webhook.");
                            println!("skipped slots {leader_slots:?}");   
                            index+=1;                 
                        }else{
                            println!("potential skipe slot {ls} {current_slot}");
                            index+=1;
                        }
                    }else if current_slot==ls {
                        completed_slots+=1;
                        leader_slots.remove(index);
                         println!("completed slots {leader_slots:?}");
                
                         break;
                    }else{
                       
                        break;
                    }
                    len_vec=leader_slots.len();  
                }

            }
            Err(e) => {println!("grpc issue");
                println!("{e:?}");
                continue
                }
        }

        

    }
    (completed_slots,skipped_slots,unknown_slots,total_slots as i32)

}
fn intrcptr(request: tonic::Request<()>) -> Result<tonic::Request<()>, Status> {
        // request.metadata_mut();
//            .insert("access-token", self.access_token.parse().unwrap());
        Ok(request)
}



