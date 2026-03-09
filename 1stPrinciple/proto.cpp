#include <iostream>
#include <vector>
#include <map>
#include <string>
#include <cmath>
#include <ctime>
#include <iomanip>
#include <algorithm>
#include <random>

using namespace std;

class LotteryProtocol {
private:
    double k;          // Price slope
    double p0;         // Starting price
    double totalSol;   // Total SOL in pool
    double totalTickets; // Total tickets issued
    time_t startTime;
    time_t endTime;

    map<string, double> userTickets;

    // Better Randomness
    mt19937 rng;

public:
    LotteryProtocol(double slope, double startPrice, int durationSeconds) {
        k = slope;
        p0 = startPrice;
        totalSol = 0;
        totalTickets = 0;
        startTime = time(0);
        endTime = startTime + durationSeconds;
        
        // Seed RNG with current time
        rng.seed(time(0));

        cout << fixed << setprecision(4);
        cout << "[Protocol Initialized] Start Price: " << p0 << " SOL, Slope: " << k << endl;
        cout << "[Protocol Timeline] Starts: " << ctime(&startTime);
        cout << "[Protocol Timeline] Ends:   " << ctime(&endTime) << endl;
    }

    bool buyTicket(string userId, double solAmount) {
        time_t now = time(0);
        
        // Edge Case: Check Time
        if (now > endTime) {
            cout << "[Error] User " << userId << " tried to buy after time limit!" << endl;
            return false;
        }

        // Edge Case: Invalid Amount
        if (solAmount <= 0) {
            cout << "[Error] Invalid SOL amount: " << solAmount << endl;
            return false;
        }

        double deltaTickets = 0;
        double newTotalSol = totalSol + solAmount;

        // Bonding Curve Logic: S(T) = 0.5*k*T^2 + p0*T
        // Solving for T: T = (-p0 + sqrt(p0^2 + 2*k*S)) / k
        if (k > 0) {
            double newTotalTickets = (-p0 + sqrt(p0 * p0 + 2 * k * newTotalSol)) / k;
            deltaTickets = newTotalTickets - totalTickets;
        } else {
            // Flat price case (k=0)
            deltaTickets = solAmount / p0;
        }

        // Update State
        userTickets[userId] += deltaTickets;
        totalTickets += deltaTickets;
        totalSol = newTotalSol;

        cout << "[Purchase] " << userId << " spent " << solAmount << " SOL -> Got " << deltaTickets << " tickets." << endl;
        return true;
    }

    string drawWinner() {
        time_t now = time(0);

        // Edge Case: Early Draw
        if (now < endTime) {
            cout << "[Warning] Drawing before end time! (Remaining: " << endTime - now << "s)" << endl;
        }

        // Edge Case: No participants
        if (totalTickets <= 0) {
            return "NO_PARTICIPANTS";
        }

        // High-quality Random selection (Weighted by tickets)
        uniform_real_distribution<double> dist(0, totalTickets);
        double roll = dist(rng);
        double cumulative = 0;

        for (auto const& entry : userTickets) {
            double tickets = entry.second;
            cumulative += tickets;
            if (roll <= cumulative) {
                return entry.first;
            }
        }

        return "UNKNOWN_ERROR";
    }

    void displayStatus() {
        cout << "\n--- Current Pool Status ---" << endl;
        cout << "Total SOL:     " << totalSol << endl;
        cout << "Total Tickets: " << totalTickets << endl;
        cout << "Participants:  " << userTickets.size() << endl;
        
        for (auto const& entry : userTickets) {
            string user = entry.first;
            double tickets = entry.second;
            double prob = (tickets / totalTickets) * 100.0;
            cout << "  - " << user << ": " << tickets << " tickets (" << prob << "%)" << endl;
        }
        cout << "---------------------------\n" << endl;
    }
};

int main() {
    // Initialize with 0.1 slope, 1.0 start price, 10 second duration
    LotteryProtocol lottery(0.1, 1.0, 10);

    // Simulation
    lottery.buyTicket("Alice", 1.4);  // Early buyer (cheap tickets)
    lottery.buyTicket("Al", 1.0);  // Early buyer (cheap tickets)
    lottery.buyTicket("Bob", 1.0);    // Second buyer (pricier)
    lottery.buyTicket("Charlie", 2.0);// Whale buyer
    lottery.buyTicket("App", 1.0);  // Early buyer (cheap tickets)
    
    lottery.displayStatus();

    cout << "Waiting for time limit to expire..." << endl;
    // In a real app we'd wait, here we simulate a late attempt
    // (Manual time manipulation isn't easy here, but we can call it)
    
    // Simulate drawing
    string winner = lottery.drawWinner();
    cout << "\n*** WINNER IS: " << winner << " ***" << endl;

    return 0;
}
