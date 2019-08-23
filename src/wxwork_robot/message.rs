use bytes::buf::IntoBuf;
use actix_web::web;
use openssl::hash;

// use hex;
use quick_xml::events::{BytesEnd, BytesStart, BytesText, Event};
use quick_xml::Reader;
use quick_xml::Writer;
use std::io::Cursor;

use actix_web::HttpResponse;

use regex::{Regex, RegexBuilder};

use super::base64;

#[derive(Debug, Clone)]
pub struct WXWorkMessageDec {
    pub content: String,
    pub receiveid: String,
}

#[derive(Debug, Clone)]
pub struct WXWorkMessageFrom {
    pub user_id: String,
    pub name: String,
    pub alias: String,
}

#[derive(Debug, Clone)]
pub struct WXWorkMessageNtf {
    pub web_hook_key: String,
    pub web_hook_url: String,
    pub from: WXWorkMessageFrom,
    pub msg_type: String,
    pub content: String,
    pub msg_id: String,
    pub chat_id: String,
    pub chat_type: String,
    pub get_chat_info_url: String,
}

#[derive(Debug, Clone)]
pub struct WXWorkMessageTextRsp {
    pub content: String,
    pub mentioned_list: Vec<String>,
    pub mentioned_mobile_list: Vec<u64>,
}

#[derive(Debug, Clone)]
pub struct WXWorkMessageMarkdownRsp {
    pub content: String,
}

#[derive(Debug, Clone)]
pub struct WXWorkMessageImageRsp {
    pub content: Vec<u8>,
}

lazy_static! {
    static ref PICK_WEBHOOK_KEY_RULE: Regex = RegexBuilder::new("key=(?P<KEY>[\\d\\w\\-_]+)")
        .case_insensitive(false)
        .build()
        .unwrap();
}

pub fn get_msg_encrypt_from_bytes(bytes: web::Bytes) -> Option<String> {
    let mut reader = Reader::from_reader(bytes.into_buf());
    reader.trim_text(true);
    let mut is_msg_field = false;
    let mut ret = None;
    let mut buf = Vec::new();
    loop {
        match reader.read_event(&mut buf) {
            Ok(Event::Start(ref e)) => match e.name() {
                b"Encrypt" => {
                    is_msg_field = true;
                }
                _ => (),
            },
            Ok(Event::End(ref e)) => match e.name() {
                b"Encrypt" => {
                    is_msg_field = false;
                }
                _ => (),
            },
            Ok(Event::CData(data)) => {
                if is_msg_field {
                    if let Ok(x) = data.unescaped() {
                        match String::from_utf8(Vec::from(x)) {
                            Ok(s) => {
                                ret = Some(s);
                            }
                            Err(e) => {
                                error!("decode Encrypt as utf8 failed, {:?}", e);
                            }
                        }
                    }
                }
            }
            Err(e) => error!("Error at position {}: {:?}", reader.buffer_position(), e),
            Ok(Event::Eof) => break,
            _ => (),
        }
        buf.clear();
    }

    ret
}

enum WXWorkMsgField {
    NONE,
    WebHookUrl,
    FromUserId,
    FromName,
    FromAlias,
    MsgType,
    Content,
    MsgId,
    GetChatInfoUrl,
    ChatId,
    ChatType,
}

pub fn get_msg_from_str(input: &str) -> Option<WXWorkMessageNtf> {
    let mut web_hook_url = String::default();
    let mut from_user_id = String::default();
    let mut from_name = String::default();
    let mut from_alias = String::default();
    let mut msg_type = String::default();
    let mut content = String::default();
    let mut msg_id = String::default();
    let mut chat_id = String::default();
    let mut chat_type = String::default();
    let mut get_chat_info_url = String::default();
    let mut is_in_from = false;
    let mut field_mode = WXWorkMsgField::NONE;

    let mut reader = Reader::from_str(input);
    reader.trim_text(true);

    let mut buf = Vec::new();
    loop {
        match reader.read_event(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let tag_name = e.name();
                match tag_name {
                    b"WebhookUrl" => {
                        field_mode = WXWorkMsgField::WebHookUrl;
                        debug!("Parse get ready for WebhookUrl");
                    }
                    b"From" => {
                        is_in_from = true;
                        debug!("Parse get ready for From");
                    }
                    b"UserId" => {
                        if is_in_from {
                            field_mode = WXWorkMsgField::FromUserId;
                            debug!("Parse get ready for From.UserId");
                        }
                    }
                    b"Name" => {
                        if is_in_from {
                            field_mode = WXWorkMsgField::FromName;
                            debug!("Parse get ready for From.Name");
                        }
                    }
                    b"Alias" => {
                        if is_in_from {
                            field_mode = WXWorkMsgField::FromAlias;
                            debug!("Parse get ready for From.Alias");
                        }
                    }
                    b"MsgType" => {
                        field_mode = WXWorkMsgField::MsgType;
                        debug!("Parse get ready for MsgType");
                    }
                    b"Text" | b"Markdown" => {
                        field_mode = WXWorkMsgField::Content;
                        debug!("Parse get ready for Content");
                    }
                    b"MsgId" => {
                        field_mode = WXWorkMsgField::MsgId;
                        debug!("Parse get ready for MsgId");
                    }
                    b"GetChatInfoUrl" => {
                        field_mode = WXWorkMsgField::GetChatInfoUrl;
                        debug!("Parse get ready for GetChatInfoUrl");
                    }
                    b"ChatId" => {
                        field_mode = WXWorkMsgField::ChatId;
                        debug!("Parse get ready for ChatId");
                    }
                    b"ChatType" => {
                        field_mode = WXWorkMsgField::ChatType;
                        debug!("Parse get ready for ChatType");
                    }
                    any => {
                        debug!(
                            "Ignore start label for {}",
                            if let Ok(x) = String::from_utf8(any.to_vec()) {
                                x
                            } else {
                                String::from("UNKNOWN")
                            }
                        );
                    }
                }
            },
            Ok(Event::End(ref e)) => match e.name() {
                b"WebhookUrl" => {
                    if let WXWorkMsgField::WebHookUrl = field_mode {
                        field_mode = WXWorkMsgField::NONE;
                        debug!("Parse close for WebhookUrl");
                    }
                }
                b"From" => {
                    is_in_from = false;
                    debug!("Parse close for From");
                }
                b"UserId" => {
                    if is_in_from {
                        if let WXWorkMsgField::FromUserId = field_mode {
                            field_mode = WXWorkMsgField::NONE;
                            debug!("Parse close for From.UserId");
                        }
                    }
                }
                b"Name" => {
                    if is_in_from {
                        if let WXWorkMsgField::FromName = field_mode {
                            field_mode = WXWorkMsgField::NONE;
                            debug!("Parse close for From.Name");
                        }
                    }
                }
                b"Alias" => {
                    if is_in_from {
                        if let WXWorkMsgField::FromAlias = field_mode {
                            field_mode = WXWorkMsgField::NONE;
                            debug!("Parse close for From.Alias");
                        }
                    }
                }
                b"MsgType" => {
                    if let WXWorkMsgField::MsgType = field_mode {
                        field_mode = WXWorkMsgField::NONE;
                        debug!("Parse close for MsgType");
                    }
                }
                b"Text" | b"Markdown" => {
                    if let WXWorkMsgField::Content = field_mode {
                        field_mode = WXWorkMsgField::NONE;
                        debug!("Parse close for Content");
                    }
                }
                b"MsgId" => {
                    if let WXWorkMsgField::MsgId = field_mode {
                        field_mode = WXWorkMsgField::NONE;
                        debug!("Parse close for MsgId");
                    }
                }
                b"GetChatInfoUrl" => {
                    if let WXWorkMsgField::GetChatInfoUrl = field_mode {
                        field_mode = WXWorkMsgField::NONE;
                        debug!("Parse close for GetChatInfoUrl");
                    }
                }
                b"ChatId" => {
                    if let WXWorkMsgField::ChatId = field_mode {
                        field_mode = WXWorkMsgField::NONE;
                        debug!("Parse close for ChatId");
                    }
                }
                b"ChatType" => {
                    if let WXWorkMsgField::ChatType = field_mode {
                        field_mode = WXWorkMsgField::NONE;
                        debug!("Parse close for ChatType");
                    }
                }
                any => {
                    debug!(
                        "Ignore close label for {}",
                        if let Ok(x) = String::from_utf8(any.to_vec()) {
                            x
                        } else {
                            String::from("UNKNOWN")
                        }
                    );
                }
            },
            Ok(Event::CData(data)) | Ok(Event::Text(data)) => {
                loop {
                    if let WXWorkMsgField::NONE = field_mode {
                        break;
                    }

                    let data_str_opt = if let Ok(x) = data.unescaped() {
                        match String::from_utf8(Vec::from(x)) {
                            Ok(s) => Some(s),
                            Err(e) => {
                                error!("decode Encrypt as utf8 failed, {:?}", e);
                                None
                            }
                        }
                    } else {
                        None
                    };

                    let data_str = if let Some(x) = data_str_opt {
                        x
                    } else {
                        break;
                    };

                    match field_mode {
                        WXWorkMsgField::WebHookUrl => {
                            web_hook_url = data_str;
                            debug!("Parse data for WebhookUrl");
                        }
                        WXWorkMsgField::FromUserId => {
                            from_user_id = data_str;
                            debug!("Parse data for From.UserId");
                        }
                        WXWorkMsgField::FromName => {
                            from_name = data_str;
                            debug!("Parse data for From.Name");
                        }
                        WXWorkMsgField::FromAlias => {
                            from_alias = data_str;
                            debug!("Parse data for From.Alias");
                        }
                        WXWorkMsgField::MsgType => {
                            msg_type = data_str;
                            debug!("Parse data for MsgType");
                        }
                        WXWorkMsgField::Content => {
                            content = data_str;
                            debug!("Parse data for Content");
                        }
                        WXWorkMsgField::MsgId => {
                            msg_id = data_str;
                            debug!("Parse data for MsgId");
                        }
                        WXWorkMsgField::GetChatInfoUrl => {
                            get_chat_info_url = data_str;
                            debug!("Parse data for GetChatInfoUrl");
                        }
                        WXWorkMsgField::ChatId => {
                            chat_id = data_str;
                            debug!("Parse data for ChatId");
                        }
                        WXWorkMsgField::ChatType => {
                            chat_type = data_str;
                            debug!("Parse data for ChatId");
                        }
                        _ => {
                            debug!("Ignore data {}", data_str);
                        }
                    }

                    break;
                }
            }
            Err(e) => error!("Error at position {}: {:?}", reader.buffer_position(), e),
            Ok(Event::Eof) => break,
            _ => {}
        }
        buf.clear();
    }

    let web_hook_key = if let Some(caps) = PICK_WEBHOOK_KEY_RULE.captures(web_hook_url.as_str()) {
        if let Some(x) = caps.name("KEY") {
            String::from(x.as_str())
        } else {
            String::default()
        }
    } else {
        String::default()
    };

    if 0 == web_hook_key.len() {
        error!("We can not get robot key from {}", web_hook_url);
    }

    Some(WXWorkMessageNtf {
        web_hook_key: web_hook_key,
        web_hook_url: web_hook_url,
        from: WXWorkMessageFrom {
            user_id: from_user_id,
            name: from_name,
            alias: from_alias,
        },
        msg_type: msg_type,
        content: content,
        msg_id: msg_id,
        chat_id: chat_id,
        chat_type: chat_type,
        get_chat_info_url: get_chat_info_url,
    })
}

pub fn pack_text_message(msg: WXWorkMessageTextRsp) -> Result<String, String> {
    debug!("{:?}", msg);
    let mut writer = Writer::new(Cursor::new(Vec::new()));

    if let Ok(_) = writer.write_event(Event::Start(BytesStart::borrowed_name(b"xml"))) {
        if let Ok(_) = writer.write_event(Event::Start(BytesStart::borrowed_name(b"MsgType"))) {
            let _ = writer.write_event(Event::CData(BytesText::from_plain_str("text")));
            let _ = writer.write_event(Event::End(BytesEnd::borrowed(b"MsgType")));
        }

        if let Ok(_) = writer.write_event(Event::Start(BytesStart::borrowed_name(b"Text"))) {
            if let Ok(_) = writer.write_event(Event::Start(BytesStart::borrowed_name(b"Content"))) {
                let _ = writer.write_event(Event::CData(BytesText::from_plain_str(
                    msg.content.as_str(),
                )));
                let _ = writer.write_event(Event::End(BytesEnd::borrowed(b"Content")));
            }

            if let Ok(_) =
                writer.write_event(Event::Start(BytesStart::borrowed_name(b"MentionedList")))
            {
                for v in msg.mentioned_list {
                    if let Ok(_) =
                        writer.write_event(Event::Start(BytesStart::borrowed_name(b"Item")))
                    {
                        let _ =
                            writer.write_event(Event::CData(BytesText::from_plain_str(v.as_str())));
                        let _ = writer.write_event(Event::End(BytesEnd::borrowed(b"Item")));
                    }
                }
                let _ = writer.write_event(Event::End(BytesEnd::borrowed(b"MentionedList")));
            }

            if let Ok(_) = writer.write_event(Event::Start(BytesStart::borrowed_name(
                b"MentionedMobileList",
            ))) {
                for v in msg.mentioned_mobile_list {
                    if let Ok(_) =
                        writer.write_event(Event::Start(BytesStart::borrowed_name(b"Item")))
                    {
                        let _ = writer.write_event(Event::CData(BytesText::from_plain_str(
                            v.to_string().as_str(),
                        )));
                        let _ = writer.write_event(Event::End(BytesEnd::borrowed(b"Item")));
                    }
                }
                let _ = writer.write_event(Event::End(BytesEnd::borrowed(b"MentionedMobileList")));
            }

            let _ = writer.write_event(Event::End(BytesEnd::borrowed(b"Text")));
        }
        let _ = writer.write_event(Event::End(BytesEnd::borrowed(b"xml")));
    }

    match String::from_utf8(writer.into_inner().into_inner()) {
        Ok(ret) => Ok(ret),
        Err(e) => Err(format!("{:?}", e)),
    }
}

pub fn pack_markdown_message(msg: WXWorkMessageMarkdownRsp) -> Result<String, String> {
    debug!("{:?}", msg);
    let mut writer = Writer::new(Cursor::new(Vec::new()));

    if let Ok(_) = writer.write_event(Event::Start(BytesStart::borrowed_name(b"xml"))) {
        if let Ok(_) = writer.write_event(Event::Start(BytesStart::borrowed_name(b"MsgType"))) {
            let _ = writer.write_event(Event::CData(BytesText::from_plain_str("markdown")));
            let _ = writer.write_event(Event::End(BytesEnd::borrowed(b"MsgType")));
        }

        if let Ok(_) = writer.write_event(Event::Start(BytesStart::borrowed_name(b"Markdown"))) {
            if let Ok(_) = writer.write_event(Event::Start(BytesStart::borrowed_name(b"Content"))) {
                // BytesText::from_escaped_str
                let _ = writer.write_event(Event::CData(BytesText::from_escaped_str(
                    msg.content.as_str(),
                )));
                let _ = writer.write_event(Event::End(BytesEnd::borrowed(b"Content")));
            }

            let _ = writer.write_event(Event::End(BytesEnd::borrowed(b"Markdown")));
        }
        let _ = writer.write_event(Event::End(BytesEnd::borrowed(b"xml")));
    }

    match String::from_utf8(writer.into_inner().into_inner()) {
        Ok(ret) => Ok(ret),
        Err(e) => Err(format!("{:?}", e)),
    }
}

pub fn pack_image_message(msg: WXWorkMessageImageRsp) -> Result<String, String> {
    debug!("{:?}", msg);
    let mut writer = Writer::new(Cursor::new(Vec::new()));

    if let Ok(_) = writer.write_event(Event::Start(BytesStart::borrowed_name(b"xml"))) {
        if let Ok(_) = writer.write_event(Event::Start(BytesStart::borrowed_name(b"MsgType"))) {
            let _ = writer.write_event(Event::CData(BytesText::from_plain_str("image")));
            let _ = writer.write_event(Event::End(BytesEnd::borrowed(b"MsgType")));
        }

        if let Ok(_) = writer.write_event(Event::Start(BytesStart::borrowed_name(b"Image"))) {
            if let Ok(_) = writer.write_event(Event::Start(BytesStart::borrowed_name(b"Base64"))) {
                // BytesText::from_escaped_str
                let _ = writer.write_event(Event::CData(BytesText::from_escaped_str(
                    match base64::STANDARD.encode(&msg.content) {
                        Ok(x) => x,
                        Err(e) => e.message
                    }
                )));
                let _ = writer.write_event(Event::End(BytesEnd::borrowed(b"Base64")));
            }

            match hash::hash(hash::MessageDigest::md5(), &msg.content) {
                Ok(x) => {
                    if let Ok(_) =
                        writer.write_event(Event::Start(BytesStart::borrowed_name(b"Md5")))
                    {
                        // BytesText::from_escaped_str
                        let _ = writer.write_event(Event::CData(BytesText::from_escaped_str(
                            hex::encode(x.as_ref()).as_str(),
                        )));
                        let _ = writer.write_event(Event::End(BytesEnd::borrowed(b"Md5")));
                    }
                }
                Err(e) => error!("Md5 for {} failed, {:?}", hex::encode(&msg.content), e),
            }

            let _ = writer.write_event(Event::End(BytesEnd::borrowed(b"Image")));
        }
        let _ = writer.write_event(Event::End(BytesEnd::borrowed(b"xml")));
    }

    match String::from_utf8(writer.into_inner().into_inner()) {
        Ok(ret) => Ok(ret),
        Err(e) => Err(format!("{:?}", e)),
    }
}

pub fn pack_message_response(
    encrypt: String,
    msg_signature: String,
    timestamp: String,
    nonce: String,
) -> Result<String, String> {
    let mut writer = Writer::new(Cursor::new(Vec::new()));

    if let Ok(_) = writer.write_event(Event::Start(BytesStart::borrowed_name(b"xml"))) {
        if let Ok(_) = writer.write_event(Event::Start(BytesStart::borrowed_name(b"Encrypt"))) {
            let _ = writer.write_event(Event::CData(BytesText::from_plain_str(encrypt.as_str())));
            let _ = writer.write_event(Event::End(BytesEnd::borrowed(b"Encrypt")));
        }

        if let Ok(_) = writer.write_event(Event::Start(BytesStart::borrowed_name(b"MsgSignature")))
        {
            let _ = writer.write_event(Event::CData(BytesText::from_plain_str(
                msg_signature.as_str(),
            )));
            let _ = writer.write_event(Event::End(BytesEnd::borrowed(b"MsgSignature")));
        }

        if let Ok(_) = writer.write_event(Event::Start(BytesStart::borrowed_name(b"TimeStamp"))) {
            let _ = writer.write_event(Event::Text(BytesText::from_plain_str(timestamp.as_str())));
            let _ = writer.write_event(Event::End(BytesEnd::borrowed(b"TimeStamp")));
        }

        if let Ok(_) = writer.write_event(Event::Start(BytesStart::borrowed_name(b"Nonce"))) {
            let _ = writer.write_event(Event::CData(BytesText::from_plain_str(nonce.as_str())));
            let _ = writer.write_event(Event::End(BytesEnd::borrowed(b"Nonce")));
        }

        let _ = writer.write_event(Event::End(BytesEnd::borrowed(b"xml")));
    }

    match String::from_utf8(writer.into_inner().into_inner()) {
        Ok(ret) => {
            debug!("packed encrypted message success\n{}", ret);
            Ok(ret)
        }
        Err(e) => {
            error!("packed encrypted message failed: {:?}", e);
            Err(format!("{:?}", e))
        }
    }
}

pub fn get_robot_response_access_deny_content(msg: &str) -> String {
    error!("[Response Error]: {}", msg);
    let mut writer = Writer::new(Cursor::new(Vec::new()));

    if let Ok(_) = writer.write_event(Event::Start(BytesStart::borrowed_name(b"xml"))) {
        if let Ok(_) = writer.write_event(Event::Start(BytesStart::borrowed_name(b"message"))) {
            let _ = writer.write_event(Event::CData(BytesText::from_plain_str(msg)));
            let _ = writer.write_event(Event::End(BytesEnd::borrowed(b"message")));
        }

        if let Ok(_) = writer.write_event(Event::Start(BytesStart::borrowed_name(b"code"))) {
            let _ = writer.write_event(Event::Text(BytesText::from_plain_str("Access Deny")));
            let _ = writer.write_event(Event::End(BytesEnd::borrowed(b"code")));
        }
        let _ = writer.write_event(Event::End(BytesEnd::borrowed(b"xml")));
    }

    match String::from_utf8(writer.into_inner().into_inner()) {
        Ok(ret) => ret,
        Err(e) => format!("{:?}", e),
    }
}

pub fn get_robot_response_access_deny(msg: String) -> String {
    get_robot_response_access_deny_content(msg.as_str())
}

pub fn make_robot_error_response_content(msg: &str) -> HttpResponse {
    HttpResponse::Forbidden()
        .content_type("application/xml")
        .body(get_robot_response_access_deny_content(msg))
}

pub fn make_robot_error_response(msg: String) -> HttpResponse {
    make_robot_error_response_content(msg.as_str())
}

pub fn make_robot_not_found_response_content(msg: &str) -> HttpResponse {
    HttpResponse::NotFound()
        .content_type("application/xml")
        .body(get_robot_response_access_deny_content(msg))
}

pub fn make_robot_not_found_response(msg: String) -> HttpResponse {
    make_robot_not_found_response_content(msg.as_str())
}


#[cfg(test)]
mod tests {

    use super::*;

    const WXWORKROBOT_TEST_MSG: &str = "<xml><From><UserId><![CDATA[T56650002A]]></UserId><Name><![CDATA[欧文韬]]></Name><Alias><![CDATA[owentou]]></Alias></From><WebhookUrl><![CDATA[http://in.qyapi.weixin.qq.com/cgi-bin/webhook/send?key=xxxxxxxxxxxx]]></WebhookUrl><ChatId><![CDATA[fakechatid]]></ChatId><GetChatInfoUrl><![CDATA[http://in.qyapi.weixin.qq.com/cgi-bin/webhook/get_chat_info?code=VcgjNN2bHMhatXwG8aZbHvj_RZmLF0OSS5_sVGxYUGk]]></GetChatInfoUrl><MsgId><![CDATA[CIGABBCOgP3qBRiR4vm7goCAAyAY]]></MsgId><ChatType><![CDATA[group]]></ChatType><MsgType><![CDATA[text]]></MsgType><Text><Content><![CDATA[@fa机器人 help]]></Content></Text></xml>";
    const WXWORKROBOT_TEST_MSG_WITH_KNOWN_DATA: &str = "<xml><unknown_field1><![CDATA[blablabla]]></unknown_field1><From><UserId><![CDATA[T56650002A]]></UserId><Name><![CDATA[欧文韬]]></Name><Alias><![CDATA[owentou]]></Alias></From><WebhookUrl><![CDATA[http://in.qyapi.weixin.qq.com/cgi-bin/webhook/send?key=xxxxxxxxxxxx]]></WebhookUrl><ChatId><![CDATA[fakechatid]]></ChatId><unknown_field2>test_message</unknown_field2><GetChatInfoUrl><![CDATA[http://in.qyapi.weixin.qq.com/cgi-bin/webhook/get_chat_info?code=VcgjNN2bHMhatXwG8aZbHvj_RZmLF0OSS5_sVGxYUGk]]></GetChatInfoUrl><MsgId><![CDATA[CIGABBCOgP3qBRiR4vm7goCAAyAY]]></MsgId><ChatType><![CDATA[group]]></ChatType><MsgType><![CDATA[text]]></MsgType><Text><Content><![CDATA[@fa机器人 help]]></Content></Text></xml>";

    #[test]
    fn decode_wxwork_robot_msg() {
        let decode_res = get_msg_from_str(WXWORKROBOT_TEST_MSG);
        assert!(decode_res.is_some());
        if let Some(msg) = decode_res {
            assert_eq!(msg.content, "@fa机器人 help");
            assert_eq!(msg.from.user_id, "T56650002A");
            assert_eq!(msg.from.name, "欧文韬");
            assert_eq!(msg.from.alias, "owentou");
            assert_eq!(msg.msg_id, "CIGABBCOgP3qBRiR4vm7goCAAyAY");
            assert_eq!(msg.msg_type, "text");
            assert_eq!(msg.chat_id, "fakechatid");
            assert_eq!(msg.chat_type, "group");
        }
    }

    #[test]
    fn decode_wxwork_robot_msg_with_known_fields() {
        let decode_res = get_msg_from_str(WXWORKROBOT_TEST_MSG_WITH_KNOWN_DATA);
        assert!(decode_res.is_some());
        if let Some(msg) = decode_res {
            assert_eq!(msg.content, "@fa机器人 help");
            assert_eq!(msg.from.user_id, "T56650002A");
            assert_eq!(msg.from.name, "欧文韬");
            assert_eq!(msg.from.alias, "owentou");
            assert_eq!(msg.msg_id, "CIGABBCOgP3qBRiR4vm7goCAAyAY");
            assert_eq!(msg.msg_type, "text");
            assert_eq!(msg.chat_id, "fakechatid");
            assert_eq!(msg.chat_type, "group");
        }
    }
}
