use crate::messages::Message;
use crate::{styles, App, MUTED};
use iced::widget::{button, column, container, row, text, text_input, Space};
use iced::{Element, Fill};
use iota_wallet_core::display::format_balance;

impl App {
    pub(crate) fn view_send(&self) -> Element<Message> {
        if self.wallet_info.is_none() {
            return text("No wallet loaded").into();
        }

        let title = text("Send IOTA").size(24);

        let bal_label = match self.balance {
            Some(b) => format!("Available: {}", format_balance(b)),
            None => "Balance: loading...".into(),
        };

        let recipient = text_input("Recipient address or .iota name", &self.recipient)
            .on_input(Message::RecipientChanged);

        // Show resolved address or error below the input
        let resolved_hint: Option<Element<Message>> = match &self.resolved_recipient {
            Some(Ok(addr)) => Some(
                text(format!("Resolved: {addr}"))
                    .size(11)
                    .color(styles::ACCENT)
                    .into(),
            ),
            Some(Err(e)) => Some(
                text(e.as_str()).size(11).color(styles::DANGER).into(),
            ),
            None => None,
        };

        let amount = text_input("Amount (IOTA)", &self.amount)
            .on_input(Message::AmountChanged)
            .on_submit(Message::ConfirmSend);

        let mut send = button(text("Send").size(14))
            .padding([10, 24])
            .style(styles::btn_primary);
        if self.loading == 0 && !self.recipient.is_empty() && !self.amount.is_empty() {
            send = send.on_press(Message::ConfirmSend);
        }

        let mut form = column![
            text(bal_label).size(14).font(styles::BOLD),
            Space::new().height(8),
            text("Recipient").size(12).color(MUTED),
            recipient,
        ]
        .spacing(4);
        if let Some(hint) = resolved_hint {
            form = form.push(hint);
        }
        form = form
            .push(Space::new().height(4))
            .push(text("Amount").size(12).color(MUTED))
            .push(amount)
            .push(Space::new().height(12))
            .push(send);

        let header = row![title, Space::new().width(Fill)]
            .align_y(iced::Alignment::Center);

        let mut col = column![
            header,
            container(form)
                .padding(24)
                .width(Fill)
                .style(styles::card),
        ]
        .spacing(16);

        if self.loading > 0 {
            col = col.push(text("Sending...").size(13).color(MUTED));
        }
        if let Some(err) = &self.error_message {
            col = col.push(text(err.as_str()).size(13).color(styles::DANGER));
        }
        if let Some(msg) = &self.success_message {
            col = col.push(text(msg.as_str()).size(13).color(styles::ACCENT));
        }

        col.into()
    }
}
