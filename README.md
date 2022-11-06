![samotop]

[samotop]: SaMoToP.svg

# Samotop
Clone from [original](https://gitlab.com/BrightOpen/Samotop)

An SMTP [server](samotop-server/README.md) and [library](samotop/README.md) implemented in Rust.


# Inbound mail processing

[![inbound]](http://www.plantuml.com/plantuml/umla/RP1FJm913CNlyoaQJkjXYNhSWs4bnfX8GWHEXaFPgR1n_fZvenB3TxSxbAZ4sqpxw-lhcyI48MLhbCQ46um4exRhV7OfZa0fvpLNPjYRZVzSx6CYEu8l1V0ijJM_PLJ0OUwWL6Tyrj2xHC5HEiwqpgST1LXGUAUmLWUXSgHGYERMRvgYlcg7Tlb3VNCiD108DLXUeXhKjdVSos-bZGwtPCcbjVhzWJhqsrrYf5Xhm9OUeDnu1Xjw6LX9u5zyrJ9NNTO_2JJ0P_83geTPExzWPjcALk7kCmRDrDKOfZlgNk5fEbz2zJXR2dnoUQPwFGPDfkUaoicd2T63MliFzyVibXA4R2YgiwJ5CQyQ8ZIuX-fk8UjbdSX9Jcf2JcThlW40)

[inbound]: http://www.plantuml.com/plantuml/png/RP1FJm913CNlyoaQJkjXYNhSWs4bnfX8GWHEXaFPgR1n_fZvenB3TxSxbAZ4sqpxw-lhcyI48MLhbCQ46um4exRhV7OfZa0fvpLNPjYRZVzSx6CYEu8l1V0ijJM_PLJ0OUwWL6Tyrj2xHC5HEiwqpgST1LXGUAUmLWUXSgHGYERMRvgYlcg7Tlb3VNCiD108DLXUeXhKjdVSos-bZGwtPCcbjVhzWJhqsrrYf5Xhm9OUeDnu1Xjw6LX9u5zyrJ9NNTO_2JJ0P_83geTPExzWPjcALk7kCmRDrDKOfZlgNk5fEbz2zJXR2dnoUQPwFGPDfkUaoicd2T63MliFzyVibXA4R2YgiwJ5CQyQ8ZIuX-fk8UjbdSX9Jcf2JcThlW40

```
@startuml
database "Accounts" 

node "MailboxSystem" {
  [Mailbox]
  database "Mails" 
}

cloud internet {
  [Another Server]
}
cloud user {
  :Bob:
  [MUA]
}
:Bob: -> [MUA]: read mail
:admin: -up-> [Management]
[Management] -right-> Mails: manage accounts
[Management] -left-> Accounts: manage accounts
[MUA] -(0- [Mailbox]:  inbox (IMAP)
[MTA] -left(0- [Guard]: 1. Check RCPT (LMTP)
[MTA] -right(0- [Mailbox]: 2. deliver mail (LMTP)
[Guard] -down-> Accounts: get rules
[Another Server] -(0- MTA: relay (ESMTP)
[Mailbox] -down-> Mails
@enduml
```

# Outbound mail processing

[![outbound]](http://www.plantuml.com/plantuml/umla/PP11IyGm48Nl-HMFFTL35_MOWsm5RnPSggVIGzeEDj0ccime8lvtqzXTXBr--V9uRmwHJM1PPZKQDhs9XDrHI6Y7VwGQ1Y-EOuAghPkgWsgFTQVKC7iPOHrJSCJuLa1RESyJ1JGKFYXqwcUp95B8XhxtlLxD-gLQdrK6AE_-Y4OaDy8uKBaOEwjCKHRNPHAQB4Y_s1YjToWUclhvwMghLOx-qwMWKs6DcpsCy4IExM2OJbwmhnFdnBH3utQFztKrYiUSjj9pMBx7XkGjVRhOg15eDb_dCeSq8Dtq5m00)

[outbound]: http://www.plantuml.com/plantuml/png/PP11IyGm48Nl-HMFFTL35_MOWsm5RnPSggVIGzeEDj0ccime8lvtqzXTXBr--V9uRmwHJM1PPZKQDhs9XDrHI6Y7VwGQ1Y-EOuAghPkgWsgFTQVKC7iPOHrJSCJuLa1RESyJ1JGKFYXqwcUp95B8XhxtlLxD-gLQdrK6AE_-Y4OaDy8uKBaOEwjCKHRNPHAQB4Y_s1YjToWUclhvwMghLOx-qwMWKs6DcpsCy4IExM2OJbwmhnFdnBH3utQFztKrYiUSjj9pMBx7XkGjVRhOg15eDb_dCeSq8Dtq5m00

```
@startuml
database "Queue" 
database "Accounts" 

[MSA] 
[QM]

cloud internet {
  [Another Server]
}
cloud user {
  :Bob:
  [MUA]
}

:Bob: -> [MUA]: send an e-mail
[MUA] -(0- [MSA]: submission (ESMTP)
[MSA] -down-> Accounts: 1. authenticate
[MSA] -right(0- [QM]: 2. queue (LMTP)
[QM] -down-> Queue
[QM] -up(0- [Another Server]: relay (ESMTP)
@enduml
```
