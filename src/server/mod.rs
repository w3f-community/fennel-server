use crate::database::*;
use crate::rsa_tools::*;
use tokio::io::*;
use tokio::net::TcpStream;

struct FennelServerPacket {
    command: [u8; 1],
    identity: [u8; 32],
    fingerprint: [u8; 32],
    message: [u8; 1024],
    signature: [u8; 1024],
    public_key: [u8; 1024],
    recipient: [u8; 32],
}

pub async fn handle_connection(mut stream: TcpStream) -> Result<()> {
    let mut buffer = [0; 3170];
    stream.read_exact(&mut buffer).await.unwrap();
    let server_packet: FennelServerPacket = parse_packet(buffer);
    if !verify_packet_signature(&server_packet) {
        panic!("packet signature failed to verify");
    }
    if server_packet.command == [0] {
        let r = submit_identity(server_packet).await;
        stream.write_all(r).await?;
    } else if server_packet.command == [1] {
        let r = send_message(server_packet).await;
        stream.write_all(r).await?;
    } else if server_packet.command == [2] {
        let r_list = get_messages(server_packet).await;
        for r in r_list {
            stream.write_all(&r).await?;
        }
        stream.write_all(&[0]).await?;
    } else {
        stream.write_all(&[0]).await?;
    }

    Ok(())
}

fn parse_packet(buffer: [u8; 3170]) -> FennelServerPacket {
    FennelServerPacket {
        command: buffer[0..1].try_into().expect("slice with incorrect lenth"),
        identity: buffer[1..33]
            .try_into()
            .expect("slice with incorrect length"),
        fingerprint: buffer[33..65]
            .try_into()
            .expect("slice with incorrect length"),
        message: buffer[65..1089]
            .try_into()
            .expect("slice with incorrect length"),
        signature: buffer[1089..2113]
            .try_into()
            .expect("slice with incorrect length"),
        public_key: buffer[2113..3137]
            .try_into()
            .expect("slice with incorrect length"),
        recipient: buffer[3137..3169]
            .try_into()
            .expect("slice with incorrect length"),
    }
}

fn verify_packet_signature(packet: &FennelServerPacket) -> bool {
    let pub_key =
        import_keypair_from_binary(packet.public_key).expect("public key failed to import");
    verify(pub_key, packet.message.to_vec(), packet.signature.to_vec())
}

async fn submit_identity(packet: FennelServerPacket) -> &'static [u8] {
    let db = get_identity_database_handle();
    let r = insert_identity(
        db,
        Identity {
            identity_id: packet.identity,
            fingerprint: packet.fingerprint,
            public_key: packet.public_key,
        },
    );
    match r {
        Ok(_) => &[0],
        Err(_) => &[1],
    }
}

async fn send_message(packet: FennelServerPacket) -> &'static [u8] {
    let db = get_message_database_handle();
    let r = insert_message(
        db,
        Message {
            sender_id: packet.identity,
            fingerprint: packet.fingerprint,
            message: packet.message,
            signature: packet.signature,
            public_key: packet.public_key,
            recipient_id: packet.recipient,
        },
    );
    match r {
        Ok(_) => &[0],
        Err(_) => &[1],
    }
}

async fn get_messages(packet: FennelServerPacket) -> Vec<[u8; 3169]> {
    let db = get_message_database_handle();
    let messages = retrieve_messages(&db, retrieve_identity(&db, packet.identity));
    let mut result: Vec<[u8; 3169]> = Vec::new();
    for message in messages {
        result.push(message_to_bytes(&message).try_into().unwrap());
    }
    result
}
