#![cfg_attr(not(feature = "std"), no_std, no_main)]

macro_rules! ensure {
    ($expression: expr, $error: expr) => {
        if !$expression {
            return Err($error).into();
        }
    }
}

#[ink::contract]
mod dao {
    use ink::prelude::*;
    use ink::storage::traits::StorageLayout;
    use ink::storage::Mapping;
    use ink_env::hash::{HashOutput, Sha2x256};
    use scale_info::TypeInfo;
    use crate::dao::collections::BTreeMap;
    use Mapping as HashMap;

    pub type ProposalId = u32;
    pub type DaoId = u32;

    /// Event foe new proposal created
    #[ink(event)]
    pub struct ProposalCreated {
        #[ink(topic)]
        dao_id: DaoId,
        #[ink(topic)]
        proposal_id: ProposalId,
    }

    /// Event for new Vote made
    #[ink(event)]
    pub struct VoteMade {
        #[ink(topic)]
        id: (DaoId, ProposalId),
        is_in_favor: bool,
    }

    /// Event for new dao created
    #[ink(event)]
    pub struct DaoCreated {
        #[ink(topic)]
        dao_id: DaoId,
    }

    /// Event for balance transfer
    #[ink(event)]
    pub struct BalanceTransfer {
        #[ink(topic)]
        from: AccountId,
        #[ink(topic)]
        to: AccountId,
        amount: Balance,
    }

    /// Event for revealing value
    #[ink(event)]
    pub struct ValueRevealed {
        #[ink(topic)]
        account: AccountId,
        value: u64,
    }

    #[derive(scale::Encode, scale::Decode)]
    #[cfg_attr(feature = "std", derive(TypeInfo, StorageLayout))]
    pub struct DaoInfo {
        /// Who owns the dao?
        /// this can also be multi-account
        pub owner: AccountId,
        /// This dao was created at
        pub birth_block: BlockNumber,
        /// Next proposal id
        pub next_proposal_id: ProposalId,
        /// Vote cost
        pub vote_cost: Balance,
    }

    #[derive(scale::Encode, scale::Decode, Default)]
    #[cfg_attr(feature = "std", derive(TypeInfo, StorageLayout))]
    pub struct ProrposalInfo {
        /// Description of this proposal
        info: String,
        /// This proposal is created at
        created_at: BlockNumber,
        /// This proposal will be destroyed at
        destroy_at: BlockNumber,
        /// Votes in favour of this proposal
        votes_in_favour: HashMap<AccountId, Balance>,
        /// Votes against this proposal
        votes_against: HashMap<AccountId, Balance>,
    }

    #[derive(scale::Encode, scale::Decode, Default)]
    #[cfg_attr(feature = "std", derive(TypeInfo, StorageLayout))]
    pub struct RandomNumber {
        // Map to store submitted masked values
        masked_values: BTreeMap<AccountId, Vec<u8>>,
        // Map to store revealed values
        revealed_values: BTreeMap<AccountId, u64>,
        // Block height for revealing
        reveal_block_height: BlockNumber,
    }

    /// Error type
    #[derive(scale::Encode, scale::Decode)]
    #[cfg_attr(feature = "std", derive(TypeInfo, StorageLayout))]
    pub enum ContractError {
        /// Required dao does not exists
        NonExistentDao,
        /// Permission denied
        InsufficientPerimssion,
        /// Insufficient balance
        InsufficientBalance,
        /// Proposal Does not exists
        ProposalNonExistent,
        /// Vote already made
        VoteAlreadyMade,
        /// vote have not been made yet
        VoteNotYetMade,
        /// Voting has been closed
        VotingClosed,
        /// Value submitted already
        ValueAlreadySubmitted,
        /// Invalid reveal block
        InvalidRevealBlock,
        /// Value not submitted
        ValueNotSubmitted,
        /// Invalid reveal
        InvalidReveal,
    }

    pub type ContractResult = Result<(), ContractError>;

    #[ink(storage)]
    pub struct Dao {
        /// Owner of this contract
        owner: AccountId,
        /// accountInfo
        accounts: HashMap<AccountId, Balance>,
        /// Dao info
        daos: HashMap<DaoId, DaoInfo>,
        /// proposal info
        proposals: HashMap<(DaoId, ProposalId), ProrposalInfo>,
        /// next dao id
        next_dao_id: DaoId,
        /// random_number
        random_number: RandomNumber,
    }

    impl Dao {
        /// Get the current balance of this account
        pub fn get_balance(&mut self, account_id: &AccountId) -> Balance {
            self.accounts.get(account_id).unwrap_or_default()
        }

        /// increase the target's balance by amount
        pub fn increase_balance(&mut self, account_id: &AccountId, amount: Balance) {
            self.accounts
                .insert(account_id, &(self.get_balance(&account_id) + amount));
        }

        /// decrease target's balance by amount
        pub fn decrease_balance(&mut self, account_id: &AccountId, amount: Balance) {
            self.accounts
                .insert(account_id, &(self.get_balance(&account_id) - amount));
        }

        // Helper function to hash a value
        fn hash_value(&self, value: u64) -> Vec<u8> {
            let mut output = <Sha2x256 as HashOutput>::Type::default();
            ink_env::hash_bytes::<Sha2x256>(&value.to_be_bytes(), &mut output);
            output.to_vec()
        }
    }

    impl Dao {
        /// initiate new contract with specification of teh owner
        #[ink(constructor)]
        pub fn new(owner: AccountId) -> Self {
            Self {
                owner,
                next_dao_id: 1,
                accounts: Default::default(),
                daos: Default::default(),
                proposals: Default::default(),
                random_number: Default::default(),
            }
        }

        /// transfer balance from caller to target
        #[ink(message)]
        pub fn transfer(&mut self, target: AccountId, amount: Balance) -> ContractResult {
            let caller = self.env().caller();

            self.decrease_balance(&caller, amount);
            self.increase_balance(&target, amount);

            self.env().emit_event(BalanceTransfer {
                from: caller,
                to: target,
                amount,
            });

            Ok(())
        }

        /// Onwer of this contract can mint balance
        #[ink(message)]
        pub fn mint(&mut self, target: AccountId, amount: Balance) -> ContractResult  {
            let caller = self.env().caller();
            ensure!(self.owner == caller, ContractError::InsufficientPerimssion);

            self.increase_balance(&target, amount);
            Ok(())
        }

        /// Create a new dao
        #[ink(message)]
        pub fn create_dao(&mut self, owner: AccountId) -> ContractResult {
            let current_block = self.env().block_number();
            let dao_id = self.next_dao_id;

            let dao_info = DaoInfo {
                owner,
                birth_block: current_block,
                next_proposal_id: 1,
                vote_cost: 2,
            };
            self.daos.insert(dao_id, &dao_info);

            self.next_dao_id = self.next_dao_id + 1;

            self.env().emit_event(DaoCreated { dao_id });
            Ok(())
        }

        /// Create new proposal under given dao_id
        #[ink(message)]
        pub fn create_proposal(
            &mut self,
            dao_id: DaoId,
            info: String,
        ) -> Result<ProposalId, ContractError> {
            let caller = self.env().caller();
            let current_block = self.env().block_number();
            let mut dao = self
                .daos
                .get(&dao_id)
                .ok_or(ContractError::NonExistentDao)?;

            let proposal_id = dao.next_proposal_id;
            let proposal_info = ProrposalInfo {
                info,
                created_at: current_block,
                destroy_at: current_block + 1000,
                ..Default::default()
            };
            self.proposals.insert((dao_id, proposal_id), &proposal_info);

            dao.next_proposal_id = dao.next_proposal_id + 1;
            self.daos.insert(dao_id, &dao);

            self.env().emit_event(ProposalCreated {
                proposal_id,
                dao_id,
            });
            Ok(proposal_id)
        }

        /// User can vote against or in-favor of this proposal
        #[ink(message)]
        pub fn vote(&mut self, dao_id: DaoId, proposal_id: ProposalId, yes: bool) -> ContractResult {
            let caller = self.env().caller();
            let current_block = self.env().block_number();
            let mut proposal = self
                .proposals
                .get(&(dao_id, proposal_id))
                .ok_or(ContractError::NonExistentDao)?;
            let mut dao = self
                .daos
                .get(&dao_id)
                .ok_or(ContractError::NonExistentDao)?;

            let voting_power = self.get_balance(&caller);

            // make sure voter have voting cost
            ensure!(
                voting_power >= dao.vote_cost,
                ContractError::InsufficientBalance
            );
            // make sure proposal is not destroyed
            ensure!(
                proposal.destroy_at >= current_block,
                ContractError::VotingClosed
            );
            // make sure voter gave not yet made vote in-favor
            ensure!(
                !proposal.votes_in_favour.contains(&caller),
                ContractError::VoteAlreadyMade
            );
            // make sure voter gave not yet made vote against
            ensure!(
                !proposal.votes_against.contains(&caller),
                ContractError::VoteAlreadyMade
            );

            // vite in favor or against
            if yes {
                proposal.votes_in_favour.insert(caller, voting_power);
            } else {
                proposal.votes_against.insert(caller, voting_power);
            }
            // update storage
            self.proposals.insert((dao_id, proposal_id), proposal);
            // deduce the cost of voting
            self.decrease_balance(&caller, dao.vote_cost);

            self.env().emit_event(VoteMade {
                id: (dao_id, proposal_id),
                is_in_favor: yes,
            });
            Ok(())
        }

        #[ink(message)]
        pub fn balance(&mut self, account_id: AccountId) -> Balance {
            self.get_balance(&account_id)
        }

        // ALlow user to submit masked values
        #[ink(message)]
        pub fn submit_masked_value(&mut self, value_hash: Vec<u8>) -> ContractResult {
            let sender = self.env().caller();

            // Ensure that the sender has not already submitted a masked value
            ensure!(
                !self.random_number.masked_values.contains_key(&sender),
                ContractError::ValueAlreadySubmitted
            );

            // Store the masked value
            self.random_number.masked_values.insert(sender, value_hash);

            Ok(())
        }

        /// Can reveal the generated random number value
        #[ink(message)]
        pub fn reveal_value(&mut self, value: u64) -> ContractResult {
            let sender = self.env().caller();

            // Ensure that the sender has submitted a masked value
            ensure!(
                self.random_number.masked_values.contains_key(&sender),
                ContractError::ValueNotSubmitted
            );

            // Ensure that the reveal block height has been reached
            ensure!(
                self.env().block_number() >= self.random_number.reveal_block_height,
                ContractError::InvalidRevealBlock
            );

            // Verify that the revealed value matches the hashed value
            let masked_value = self.random_number.masked_values.get(&sender).unwrap();
            ensure!(
                self.hash_value(value) == *masked_value,
                ContractError::InvalidReveal
            );

            // Store the revealed value
            self.random_number.revealed_values.insert(sender, value);

            // Emit an event
            self.env().emit_event(ValueRevealed {
                account: sender,
                value,
            });
            Ok(())
        }

        /// Allow owner to set reveal block
        #[ink(message)]
        pub fn set_reveal_block_height(&mut self, block_height: BlockNumber) -> ContractResult {
            let sender = self.env().caller();

            // Only the owner can set the reveal block height
            ensure!(sender == self.owner, ContractError::InsufficientPerimssion);

            // Set the reveal block height
            self.random_number.reveal_block_height = block_height;

            Ok(())
        }
    }
}
