use crate::messages::Message;
use crate::state::Screen;
use crate::{styles, App, MUTED};
use iced::widget::{button, canvas, column, container, row, text, Space};
use iced::{Element, Fill, Length};
use iota_wallet_core::wallet::Network;

impl App {
    pub(crate) fn view_account(&self) -> Element<Message> {
        let Some(info) = &self.wallet_info else {
            return text("No wallet loaded").into();
        };

        let title = text("Account").size(24);

        let mut actions = row![
            button(text("Refresh").size(13))
                .padding([8, 16])
                .style(styles::btn_secondary)
                .on_press(Message::RefreshBalance),
        ]
        .spacing(8);

        if !info.is_mainnet && info.network_config.network != Network::Custom {
            let mut faucet = button(text("Faucet").size(13))
                .padding([8, 16])
                .style(styles::btn_secondary);
            if self.loading == 0 {
                faucet = faucet.on_press(Message::RequestFaucet);
            }
            actions = actions.push(faucet);
        }

        let header = row![title, Space::new().width(Fill), actions]
            .align_y(iced::Alignment::Center);

        let mut col = column![header].spacing(16);

        // Status messages
        if self.loading > 0 {
            col = col.push(text("Loading...").size(13).color(MUTED));
        }
        if let Some(msg) = &self.status_message {
            col = col.push(text(msg.as_str()).size(13).color(styles::ACCENT));
        }
        if let Some(msg) = &self.success_message {
            col = col.push(text(msg.as_str()).size(13).color(styles::ACCENT));
        }
        if let Some(err) = &self.error_message {
            col = col.push(text(err.as_str()).size(13).color(styles::DANGER));
        }

        // Balance chart card
        if !self.balance_chart.data.is_empty() {
            let chart_content = column![
                text("Balance History").size(16),
                canvas::Canvas::new(&self.balance_chart)
                    .width(Fill)
                    .height(Length::Fixed(200.0)),
            ]
            .spacing(12);

            col = col.push(
                container(chart_content)
                    .padding(20)
                    .width(Fill)
                    .style(styles::card),
            );
        }

        // Recent transactions card
        let mut tx_content = column![
            text("Recent Transactions").size(16),
        ]
        .spacing(12);

        if self.account_transactions.is_empty() {
            tx_content = tx_content.push(text("No transactions yet.").size(14).color(MUTED));
        } else {
            let count = self.account_transactions.len().min(5);
            tx_content =
                tx_content.push(self.view_tx_table(&self.account_transactions[..count], false));
            if self.account_transactions.len() > 5 {
                tx_content = tx_content.push(
                    button(text("View all transactions →").size(12))
                        .style(styles::btn_ghost)
                        .on_press(Message::GoTo(Screen::History)),
                );
            }
        }

        col = col.push(
            container(tx_content)
                .padding(20)
                .width(Fill)
                .style(styles::card),
        );

        col.into()
    }
}
