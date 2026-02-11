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

        let recipient = text_input("Recipient address (0x...)", &self.recipient)
            .on_input(Message::RecipientChanged);
        let amount = text_input("Amount (IOTA)", &self.amount)
            .on_input(Message::AmountChanged)
            .on_submit(Message::ConfirmSend);

        let mut send = button(text("Send").size(14))
            .padding([10, 24])
            .style(styles::btn_primary);
        if self.loading == 0 && !self.recipient.is_empty() && !self.amount.is_empty() {
            send = send.on_press(Message::ConfirmSend);
        }

        let form = column![
            text(bal_label).size(14).font(styles::BOLD),
            Space::new().height(8),
            text("Recipient").size(12).color(MUTED),
            recipient,
            Space::new().height(4),
            text("Amount").size(12).color(MUTED),
            amount,
            Space::new().height(12),
            send,
        ]
        .spacing(4);

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
