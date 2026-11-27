package invoicing;

public class Invoicing {
    public long bill(Customer customer, long total) {
        if (total <= 0) {
            throw new IllegalArgumentException("amount must be positive");
        }
        long fee = Fees.compute(total, customer.getTier());
        Ledger.debit(customer.getId(), total + fee);
        Notifications.send(customer.getEmail(), "charged", total + fee);
        return total + fee;
    }
}
