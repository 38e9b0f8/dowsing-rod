package billing;

public class Billing {
    public long charge(Account account, long amount) {
        if (amount <= 0) {
            throw new IllegalArgumentException("amount must be positive");
        }
        long fee = Fees.compute(amount, account.getTier());
        Ledger.debit(account.getId(), amount + fee);
        Notifications.send(account.getEmail(), "charged", amount + fee);
        return amount + fee;
    }
}
